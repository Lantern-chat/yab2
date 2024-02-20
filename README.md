Yet Another Backblaze B2 Client
===============================

[![crates.io](https://img.shields.io/crates/v/yab2.svg)](https://crates.io/crates/yab2)
[![Documentation](https://docs.rs/yab2/badge.svg)](https://docs.rs/yab2)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/yab2.svg)](./LICENSE-Apache)


Opinionated Backblaze B2 Client.

## Features

- Simple API making use of Rust's ownership for API constraints
- Automatic re-authentication and refreshing of Upload URLs

## Cargo Features

- `fs` (enables optimized routine for uploading from filesystem)
- `pool` (enabled non-large `UploadURL` object pool for reuse)
- `reqwest_compression` (enables deflate/gzip features on `reqwest`)