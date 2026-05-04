# sgpio_stress

Reproducer for the `sgpio_nack` / ANS2 kernel panic on MacBook Pro M1 Max.
Fills RAM and hammers the SSD, then triggers repeated hibernate/wake cycles to provoke the panic.

## How it works

1. Allocates a large RAM buffer (default 50 GB) and touches every page to force hibernation to write a full memory image.
2. Starts a background thread writing ~200 MB/s to a temp directory, keeping SSD pressure high.
3. Schedules a wake event with `pmset`, then calls `pmset sleepnow`.
4. After each wake, re-allocates RAM and repeats up to 50 times.

## Requirements

- macOS on Apple Silicon (M1/M2/M3)
- Python 3 **or** Rust toolchain (`cargo`)
- `sudo` for automatic wake scheduling (optional — see `--manual`)

## Python

```sh
# auto-wake (requires sudo)
sudo python3 sgpio_stress.py

# manual wake — open the lid or press a key after each sleep
python3 sgpio_stress.py --manual

# custom RAM size
sudo python3 sgpio_stress.py --ram-gb 32
```

## Rust

Build once, then run:

```sh
cargo build --release

# auto-wake (requires sudo)
sudo ./target/release/sgpio_stress

# manual wake
./target/release/sgpio_stress --manual

# custom RAM size
sudo ./target/release/sgpio_stress --ram-gb 32
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--ram-gb GB` | `50` | GB of RAM to allocate and keep resident |
| `--manual` | off | Skip `pmset` scheduling; wake the machine yourself |

## Notes

- The tool needs enough free RAM to satisfy `--ram-gb`. Check available memory with `vm_stat` or Activity Monitor before running.
- Temp files are written to a subdirectory of `$TMPDIR` and cleaned up on exit.
- If the machine panics (the goal), the temp directory may be left behind — safe to delete manually.
