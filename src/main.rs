use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WAKE_AFTER_SECS: u64 = 90;
const MAX_ATTEMPTS: u32 = 50;

fn allocate_ram(gb: usize) -> Vec<u8> {
    println!("Allocating {} GB RAM and touching all pages...", gb);
    let size = gb * 1024 * 1024 * 1024;
    let mut buf = vec![0xAAu8; size];
    for i in (0..size).step_by(4096) {
        buf[i] = 0x55;
    }
    println!("RAM resident ({} GB).", gb);
    buf
}

fn disk_stress(tmpdir: PathBuf, stop: Arc<AtomicBool>) {
    // pseudo-random chunk to prevent compression
    let chunk: Vec<u8> = (0..4 * 1024 * 1024)
        .map(|i: usize| (i.wrapping_mul(2654435761) ^ (i >> 16)) as u8)
        .collect();

    let mut slot = 0usize;
    while !stop.load(Ordering::Relaxed) {
        let path = tmpdir.join(format!("stress_{}.tmp", slot % 8));
        if let Ok(mut f) = fs::File::create(&path) {
            for _ in 0..50 {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if f.write_all(&chunk).is_err() {
                    break;
                }
            }
            let _ = f.flush();
            unsafe { libc::fsync(f.as_raw_fd()) };
        }
        slot += 1;
    }
}

fn schedule_wake(seconds: u64) {
    let wake_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + seconds;

    let date_out = Command::new("date")
        .args(["-r", &wake_ts.to_string(), "+%m/%d/%y %H:%M:%S"])
        .output()
        .expect("date command failed");
    let wake_str = String::from_utf8_lossy(&date_out.stdout)
        .trim()
        .to_string();

    Command::new("pmset")
        .args(["schedule", "wake", &wake_str])
        .status()
        .expect("pmset schedule wake failed");
}

fn parse_args() -> (bool, usize) {
    let args: Vec<String> = std::env::args().collect();
    let manual = args.iter().any(|a| a == "--manual");
    let ram_gb = args.windows(2)
        .find(|w| w[0] == "--ram-gb")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(50);
    (manual, ram_gb)
}

fn main() {
    let (manual, ram_gb) = parse_args();

    if !manual && unsafe { libc::geteuid() } != 0 {
        eprintln!("Run with sudo for auto-wake, or pass --manual to wake by opening the lid yourself.");
        std::process::exit(1);
    }

    let tmpdir = std::env::temp_dir().join("sgpio_stress");
    fs::create_dir_all(&tmpdir).expect("Failed to create temp dir");
    println!("Temp dir: {}", tmpdir.display());
    if manual {
        println!("Manual mode: press a key or move the mouse to wake after each sleep cycle.");
    }

    let ram = allocate_ram(ram_gb);

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let tmpdir_clone = tmpdir.clone();
    let stress_thread = thread::spawn(move || disk_stress(tmpdir_clone, stop_clone));

    println!("Disk stress running — waiting 15s for I/O to ramp up...");
    thread::sleep(Duration::from_secs(15));

    for i in 1..=MAX_ATTEMPTS {
        if manual {
            println!("\n[{}/{}] Sleeping now — press a key or move the mouse to wake...", i, MAX_ATTEMPTS);
        } else {
            println!(
                "\n[{}/{}] Scheduling wake in {}s, sleeping now...",
                i, MAX_ATTEMPTS, WAKE_AFTER_SECS
            );
            schedule_wake(WAKE_AFTER_SECS);
        }
        Command::new("pmset").arg("sleepnow").status().ok();

        // machine sleeps here, resumes on wake
        println!(
            "[{}/{}] Woke up OK. Stressing before next round...",
            i, MAX_ATTEMPTS
        );
        let _ram = allocate_ram(ram_gb);
        thread::sleep(Duration::from_secs(5));
    }

    stop.store(true, Ordering::Relaxed);
    let _ = stress_thread.join();
    let _ = fs::remove_dir_all(&tmpdir);
    println!("Done. Cleaned up.");
}
