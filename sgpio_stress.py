#!/usr/bin/env python3
"""
Reproducer for sgpio_nack / ANS2 panic on MacBook Pro M1 Max.
Triggers hibernation repeatedly under sustained SSD stress.
Usage:
  sudo python3 reproducer.py            # auto-wake via pmset schedule
       python3 reproducer.py --manual   # open lid manually to wake
"""
import os, subprocess, tempfile, time, threading, datetime, shutil, sys

WAKE_AFTER_S = 90   # seconds before scheduled auto-wake
MAX_ATTEMPTS = 50

def allocate_ram(gb):
    print(f"Allocating {gb} GB RAM and touching all pages...", flush=True)
    buf = bytearray(gb * 1024**3)
    for i in range(0, len(buf), 4096):
        buf[i] = 0xAA          # force physical page allocation
    print("RAM resident.", flush=True)
    return buf

def disk_stress(tmpdir, stop):
    chunk = os.urandom(4 * 1024 * 1024)  # 4 MB random chunk
    slot = 0
    while not stop.is_set():
        path = os.path.join(tmpdir, f"stress_{slot % 8}.tmp")
        try:
            with open(path, "wb") as f:
                for _ in range(50):              # ~200 MB per pass
                    if stop.is_set(): break
                    f.write(chunk)
                f.flush()
                os.fsync(f.fileno())
        except Exception:
            pass
        slot += 1

def schedule_wake(seconds):
    t = datetime.datetime.now() + datetime.timedelta(seconds=seconds)
    subprocess.run(
        ["pmset", "schedule", "wake", t.strftime("%m/%d/%y %H:%M:%S")],
        check=True
    )

def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--manual", action="store_true")
    parser.add_argument("--ram-gb", type=int, default=50, metavar="GB",
                        help="GB of RAM to allocate (default: 50)")
    args = parser.parse_args()

    if not args.manual and os.geteuid() != 0:
        sys.exit("Run with sudo for auto-wake, or pass --manual to wake by opening the lid yourself.")

    tmpdir = tempfile.mkdtemp(prefix="sgpio_stress_")
    print(f"Temp dir: {tmpdir}")
    if args.manual:
        print("Manual mode: press a key or move the mouse to wake after each sleep cycle.")

    ram   = allocate_ram(args.ram_gb)
    stop  = threading.Event()
    t     = threading.Thread(target=disk_stress, args=(tmpdir, stop), daemon=True)
    t.start()

    print("Disk stress running — waiting 15s for I/O to ramp up...", flush=True)
    time.sleep(15)

    try:
        for i in range(1, MAX_ATTEMPTS + 1):
            if args.manual:
                print(f"\n[{i}/{MAX_ATTEMPTS}] Sleeping now — press a key or move the mouse to wake...", flush=True)
            else:
                print(f"\n[{i}/{MAX_ATTEMPTS}] Scheduling wake in {WAKE_AFTER_S}s, sleeping now...", flush=True)
                schedule_wake(WAKE_AFTER_S)
            subprocess.run(["pmset", "sleepnow"])
            # ---- machine sleeps here; resumes below on wake ----
            print(f"[{i}/{MAX_ATTEMPTS}] Woke up OK. Stressing before next round...", flush=True)
            ram = allocate_ram(args.ram_gb)
            time.sleep(5)
    except KeyboardInterrupt:
        print("\nInterrupted.")
    finally:
        stop.set()
        shutil.rmtree(tmpdir, ignore_errors=True)
        print("Cleaned up.")

if __name__ == "__main__":
    main()
