# Setup

## Toolchain
- Rust: Homebrew `rustup`, stable toolchain (1.99.1 at creation).
  The binaries are in `/opt/homebrew/opt/rustup/bin`, which is not on the PATH by default:
  ```sh
  export PATH="/opt/homebrew/opt/rustup/bin:$PATH"   # add to ~/.zshrc
  ```
- Python: managed with `uv` (Python `>=3.11,<3.14`, same as mechaphlowers).

## Build & test
_To be completed in step 2 (maturin build)._
