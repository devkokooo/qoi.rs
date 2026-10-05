# qoi.rs
QOI codec implemented in Rust, both as a library and CLI tool

https://qoiformat.org/

## Requirements

- [x] Implement decoder
  - [x] Parse 14-byte header
    - [x] Test: missing magic bytes
    - [x] Test: normal header
    - [x] Test: malformed header
    - [x] Test: empty header
    - [x] Test: massive width & height
  - [x] Parse data chunks
    - [x] QOI_OP_RGB
    - [x] QOI_OP_RGBA
    - [x] QOI_OP_RUN
    - [x] QOI_OP_INDEX
    - [x] QOI_OP_DIFF
    - [x] QOI_OP_LUMA
- [ ] Implement encoder
  - [ ] Write 14-byte header
  - [ ] Write data chunks
    - [ ] QOI_OP_RGB
    - [ ] QOI_OP_RGBA
    - [ ] QOI_OP_RUN
    - [ ] QOI_OP_INDEX
    - [ ] QOI_OP_DIFF
    - [ ] QOI_OP_LUMA
