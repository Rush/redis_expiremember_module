# Redis ExpireMember Module

## Introduction

`redis_expiremember_module` is a custom Redis module that introduces an `EXPIREMEMBER` command. This command allows setting expiration times on individual hash fields, a feature inspired by KeyDB's `EXPIREMEMBER`, but with some distinct differences and enhancements.

## Features

- **Field-Level Expiration**: Set expiration times on individual fields within a Redis hash.
- **Custom Expiration Units**: Support for specifying expiration times in seconds (`s`) or milliseconds (`ms`).
- **Expiration Override**: Ability to update or override the expiration time for a specific field.
- **Independent Expirations**: Expirations set by `EXPIREMEMBER` are not invalidated by interacting with hash key values using other commands.
- **Manual Hash Member Removal**: If you manually delete a hash member using `HDEL`, you must also set its `expiremember` to `0` to properly manage its expiration state.

## Key Differences from KeyDB's EXPIREMEMBER

- **Independent Expiration Handling**: Unlike KeyDB, expirations set via this module are not affected by other hash operations.
- **Explicit Expiration Removal**: Requires explicit management when manually removing hash members.

## Installation

1. Clone the repository.
2. Build the module using `cargo build --release`.
3. Load the module into your Redis server.

   ```sh
   redis-server --loadmodule ./target/release/libredis_expiremember_module.so
   ```

## Usage

### Setting Expiration

```redis
EXPIREMEMBER key field time [unit]
```

- `key`: Redis hash key.
- `field`: Field within the hash to expire.
- `time`: Expiration time.
- `unit` (optional): Time unit (`s` for seconds, `ms` for milliseconds). Defaults to seconds.

### Overriding Expiration

To update the expiration time for a field, simply execute `EXPIREMEMBER` again with the new time.

### Removing Expiration

To remove expiration from a field:

```redis
EXPIREMEMBER key field 0
```

If you manually delete a field using `HDEL`, make sure to also remove its expiration.

## Example

```redis
HSET myhash field1 value1
EXPIREMEMBER myhash field1 10
```

Sets `field1` in `myhash` to expire in 10 seconds.

## Development

### Dependencies

- Rust
- Cargo (Rust's package manager)
- Redis (for running tests)

### Building

Run `cargo build` to compile the project.

### Testing

Tests are available under the `tests` module. Run them using `cargo test`. This will start a server using the `redis-server` binary.

You can also override the binary, see example below:
```
REDIS_SERVER_BIN=/sbin/redis-server cargo test
```
