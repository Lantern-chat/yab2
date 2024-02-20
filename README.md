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
- `large_buffers` (enables large buffer support, 64KiB instead of 8KiB)

## API Coverage

- [x] `b2_authorize_account`
- [x] `b2_cancel_large_file`
- [ ] `b2_copy_file`
- [ ] `b2_copy_part`
- [ ] `b2_create_bucket`
- [ ] `b2_create_key`
- [ ] `b2_delete_bucket`
- [x] `b2_delete_file_version`
- [ ] `b2_delete_key`
- [x] `b2_download_file_by_id`
- [x] `b2_download_file_by_name`
- [x] `b2_finish_large_file`
- [ ] `b2_get_download_authorization`
- [x] `b2_get_file_info`
- [x] `b2_get_upload_part_url`
- [x] `b2_get_upload_url`
- [x] `b2_hide_file`
- [ ] `b2_list_buckets`
- [x] `b2_list_file_names`
- [x] `b2_list_file_versions`
- [ ] `b2_list_keys`
- [ ] `b2_list_parts`
- [ ] `b2_list_unfinished_large_files`
- [x] `b2_start_large_file`
- [ ] `b2_update_bucket`
- [x] `b2_update_file_legal_hold`
- [ ] `b2_update_file_retention`
- [x] `b2_upload_file`
- [x] `b2_upload_part`

Missing API endpoints will be filled in over time.