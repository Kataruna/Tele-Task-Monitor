//! Process snapshot via one `NtQuerySystemInformation(SystemProcessInformation)` call.
//! Per-process CPU % is computed from these snapshots by the portable
//! [`crate::sampler::cpu::CpuTracker`].

use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows::Win32::Foundation::STATUS_INFO_LENGTH_MISMATCH;

use crate::platform::ProcessSource;
use crate::sampler::cpu::RawProc;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

/// Leading part of the kernel's SYSTEM_PROCESS_INFORMATION (x64 layout). We only read
/// these fields; the real record is longer and is followed by thread records.
#[repr(C)]
struct SpiEntry {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UnicodeString,
    base_priority: i32,
    unique_process_id: usize,
}

/// First size of the snapshot buffer (256 KiB); real machines need ~0.5-1.5 MiB, learned on the
/// first snapshot.
const INITIAL_BUF_WORDS: usize = 32 * 1024;

/// Owns the reusable snapshot buffer. It is allocated on the first snapshot, grows to what the
/// kernel asks for, and is freed by `release` while nobody is watching.
/// `Vec<u64>` guarantees the 8-byte alignment the kernel records need.
pub struct ProcSnapshot {
    buf: Vec<u64>,
    /// Largest length seen, so the buffer is re-created at the right size after a `release`.
    hint_words: usize,
}

impl ProcessSource for ProcSnapshot {
    fn new() -> Self {
        Self { buf: Vec::new(), hint_words: 0 }
    }

    /// Free the buffer (the next `take` re-allocates it at its previous size).
    fn release(&mut self) {
        self.hint_words = self.hint_words.max(self.buf.len());
        self.buf = Vec::new();
    }

    /// Take one snapshot of every process on the system.
    fn take(&mut self) -> Result<Vec<RawProc>, String> {
        if self.buf.is_empty() {
            self.buf.resize(self.hint_words.max(INITIAL_BUF_WORDS), 0);
        }
        loop {
            let mut needed = 0u32;
            let byte_len = self.buf.len() * 8;
            // SAFETY: buf is a valid writable buffer of byte_len bytes.
            let status = unsafe {
                NtQuerySystemInformation(SystemProcessInformation, self.buf.as_mut_ptr().cast(), byte_len as u32, &mut needed)
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                // Add 64 KiB headroom and grow monotonically even if `needed == 0`.
                self.buf.resize(next_buf_len(self.buf.len(), needed), 0);
                continue;
            }
            if status.is_err() {
                return Err(format!("NtQuerySystemInformation failed: {:#x}", status.0));
            }
            break;
        }

        let base = self.buf.as_ptr() as *const u8;
        let byte_len = self.buf.len() * 8;
        let mut out = Vec::with_capacity(512);
        let mut offset = 0usize;
        loop {
            if offset + std::mem::size_of::<SpiEntry>() > byte_len {
                break;
            }
            // SAFETY: the kernel wrote a chain of 8-byte-aligned records starting at offset 0,
            // each at least size_of::<SpiEntry>() bytes long; we bounds-checked above.
            let e = unsafe { &*(base.add(offset) as *const SpiEntry) };
            let name = decode_image_name(&e.image_name, base, byte_len);
            out.push(RawProc {
                pid: e.unique_process_id as u32,
                name,
                cpu_time: e.user_time + e.kernel_time,
                create_time: e.create_time,
                mem: e.working_set_private_size.max(0) as u64,
            });
            if e.next_entry_offset == 0 {
                break;
            }
            offset += e.next_entry_offset as usize;
        }
        Ok(out)
    }
}

/// Compute the next `u64` buffer length on `STATUS_INFO_LENGTH_MISMATCH`: what the kernel asked
/// for plus 64 KiB for processes that start meanwhile, and always strictly larger than now even
/// when the kernel reports `needed == 0` or a smaller hint. (Doubling would zero-fill, and so
/// commit, memory that is never used.)
fn next_buf_len(current_len: usize, needed_bytes: u32) -> usize {
    (needed_bytes as usize / 8 + 8 * 1024).max(current_len.saturating_add(8 * 1024))
}

/// Decode `SpiEntry::image_name` only when `buffer` is non-null, 2-byte aligned, and
/// completely contained within `[base .. base + byte_len]`.
fn decode_image_name(us: &UnicodeString, base: *const u8, byte_len: usize) -> String {
    let ptr = us.buffer as *const u8;
    let len_bytes = (us.length as usize) & !1;
    if ptr.is_null() || len_bytes == 0 || (ptr as usize) % 2 != 0 {
        return String::new();
    }
    let base_addr = base as usize;
    let end_addr = base_addr.saturating_add(byte_len);
    let str_start = ptr as usize;
    let Some(str_end) = str_start.checked_add(len_bytes) else {
        return String::new();
    };
    if str_start < base_addr || str_end > end_addr {
        return String::new();
    }
    // SAFETY: `us.buffer` is non-null, 2-byte aligned, and `[ptr .. ptr + len_bytes]` lies inside `buf`.
    let s = unsafe { std::slice::from_raw_parts(us.buffer, len_bytes / 2) };
    String::from_utf16_lossy(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_this_process() {
        let mut snap = ProcSnapshot::new();
        let procs = snap.take().expect("snapshot");
        let me = std::process::id();
        let mine = procs.iter().find(|p| p.pid == me).expect("own pid present");
        assert!(mine.name.to_lowercase().ends_with(".exe"), "name was {:?}", mine.name);
        assert!(mine.mem > 0);
        assert!(procs.len() > 20);
    }

    #[test]
    fn released_snapshot_buffer_comes_back_at_its_old_size() {
        let mut snap = ProcSnapshot::new();
        let before = snap.take().expect("snapshot").len();
        let grown = snap.buf.len();
        snap.release();
        assert!(snap.buf.is_empty(), "released memory is freed");
        let again = snap.take().expect("snapshot after release");
        assert!(again.iter().any(|p| p.pid == std::process::id()));
        assert!(before > 20 && again.len() > 20);
        assert!(snap.buf.len() >= grown.min(snap.hint_words), "re-created at the learned size");
    }

    #[test]
    fn buffer_growth_is_monotonic_and_bounds_check_rejects_bad_pointers() {
        // Needed == 0 still grows (by a fixed step), never shrinks and never stalls.
        assert_eq!(next_buf_len(128 * 1024, 0), 136 * 1024);
        // A smaller hint than the current size also grows.
        assert!(next_buf_len(128 * 1024, 1024) > 128 * 1024);
        // The kernel's request plus 64 KiB headroom, not a doubling.
        assert_eq!(next_buf_len(32 * 1024, 4 * 1024 * 1024), 4 * 1024 * 1024 / 8 + 8 * 1024);

        let backing: [u16; 4] = [b'a' as u16, b'b' as u16, b'c' as u16, 0];
        let base = backing.as_ptr() as *const u8;
        let byte_len = std::mem::size_of_val(&backing);

        let valid = UnicodeString { length: 6, maximum_length: 8, buffer: backing.as_ptr() };
        assert_eq!(decode_image_name(&valid, base, byte_len), "abc");

        let out_of_bounds = UnicodeString { length: 16, maximum_length: 16, buffer: backing.as_ptr() };
        assert_eq!(decode_image_name(&out_of_bounds, base, byte_len), "");

        let unaligned_ptr = unsafe { base.add(1) as *const u16 };
        let unaligned = UnicodeString { length: 4, maximum_length: 4, buffer: unaligned_ptr };
        assert_eq!(decode_image_name(&unaligned, base, byte_len), "");
    }
}
