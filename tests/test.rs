#[cfg(test)]
mod tests {
    use redis::RedisResult;
    use std::process::{Command, Child};
    use std::env;
    use std::sync::{Arc, Mutex, atomic::AtomicUsize};
    use std::time::Duration;
    use ctor::{ctor, dtor};
    use lazy_static::lazy_static;
    use std::time::Instant;

    lazy_static! {
        static ref REDIS_SERVER: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        static ref TEST_COUNT: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    }

    #[ctor]
    fn setup_redis_server() {
        let redis_server_bin = env::var("REDIS_SERVER_BIN").unwrap_or_else(|_| "redis-server".to_string());
        let child = Command::new(redis_server_bin)
            .arg("--port")
            .arg("34123")
            .arg("--loadmodule")
            .arg("target/debug/libredis_expiremember_module.so")
            .spawn()
            .expect("Failed to start Redis server with the module");

        std::thread::sleep(Duration::from_secs(1)); // Wait for the server to start
        *REDIS_SERVER.lock().unwrap() = Some(child);
    }

    #[dtor]
    fn teardown_redis_server() {
        if let Some(mut server) = REDIS_SERVER.lock().unwrap().take() {
            let _ = server.kill();
        }
    }

    #[test]
    fn test_expiremember_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        // Set a field in a hash using `redis::cmd`
        let _: () = redis::cmd("HSET")
                        .arg("myhash")
                        .arg("field1")
                        .arg("value1")
                        .query(&mut con)?;

        // Set expiration for the field using custom `EXPIREMEMBER` command
        let _: () = redis::cmd("EXPIREMEMBER")
                        .arg("myhash")
                        .arg("field1")
                        .arg(2) // 2 seconds expiration
                        .query(&mut con)?;
        std::thread::sleep(Duration::from_secs(1));

        let exists: u8 = redis::cmd("HEXISTS")
                        .arg("myhash")
                        .arg("field1")
                        .query(&mut con)?;
        assert!(exists == 1, "The field should still exist at this point in time");
                        
        // Wait for more than 2 seconds
        std::thread::sleep(Duration::from_secs(2));

        // Check if the field is deleted
        let exists: u8 = redis::cmd("HEXISTS")
                            .arg("myhash")
                            .arg("field1")
                            .query(&mut con)?;
        assert!(exists == 0, "The field should be deleted after expiration");

        Ok(())
    }

    #[test]
    fn test_expiremember_bulk_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        const NUM_FIELDS: usize = 1000;
        const MAX_EXPIRATION: u64 = 10; // Max expiration time in seconds

        // Check for expirations every second and assert presence of non-expired fields
        
        // Set fields in a hash with varying expiration times
        for i in 0..NUM_FIELDS {
            let field = format!("field{}", i);
            let expire_in = (i % MAX_EXPIRATION as usize) as u64 + 1; // Expiration time between 1 to MAX_EXPIRATION seconds

            // Set the field
            let _: () = redis::cmd("HSET")
                .arg(format!("myhash{}", expire_in / 2))
                .arg(&field)
                .arg("value")
                .query(&mut con)?;

            // Set the expiration
            let _: () = redis::cmd("EXPIREMEMBER")
            .arg(format!("myhash{}", expire_in / 2))
                .arg(&field)
                .arg(expire_in)
                .query(&mut con)?;
        }
        let start = Instant::now();

        for sec in 0..MAX_EXPIRATION {
          while start.elapsed().as_millis() < (sec * 1000 + 500) as u128 {
              std::thread::sleep(Duration::from_millis(50));
          }

          for i in 0..NUM_FIELDS {
              let field = format!("field{}", i);
              let expiration_sec = (i % MAX_EXPIRATION as usize) as u64 + 1;
              let exists: u8 = redis::cmd("HEXISTS")
                  .arg(format!("myhash{}", expiration_sec / 2))
                  .arg(&field)
                  .query(&mut con)?;

              if start.elapsed().as_millis() > 1000 * expiration_sec as u128 {
                  assert!(exists == 0, "Field {} with expiry {}s should have expired after {} msecs", field, expiration_sec, start.elapsed().as_millis());
              } else {
                  assert!(exists == 1, "Field {} with expiry {}s should still exist at {} msecs", field, expiration_sec, start.elapsed().as_millis());
              }
          }
      }

      // One final check after MAX_EXPIRATION seconds
      std::thread::sleep(Duration::from_secs(1));
      for i in 0..NUM_FIELDS {
          let expiration_sec = (i % MAX_EXPIRATION as usize) as u64 + 1;
          let field = format!("field{}", i);
          let exists: u8 = redis::cmd("HEXISTS")
              .arg(format!("myhash{}", expiration_sec / 2))
              .arg(&field)
              .query(&mut con)?;
          assert!(exists == 0, "Field {} should have expired after MAX_EXPIRATION seconds", field);
      }

        Ok(())
    }
    
    #[test]
    fn test_expiremember_overriding_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        // Set a field in a hash
        let _: () = redis::cmd("HSET")
            .arg("myhash3")
            .arg("field")
            .arg("value")
            .query(&mut con)?;

        // Initially set the expiration to 5 seconds
        let _: () = redis::cmd("EXPIREMEMBER")
            .arg("myhash3")
            .arg("field")
            .arg(5)
            .query(&mut con)?;

        std::thread::sleep(Duration::from_secs(1));

        // Override the expiration to 2 seconds
        let _: () = redis::cmd("EXPIREMEMBER")
            .arg("myhash3")
            .arg("field")
            .arg(2)
            .query(&mut con)?;

        // Wait for more than 2 seconds but less than 5 seconds
        std::thread::sleep(Duration::from_secs(3));

        // Check if the field is deleted
        let exists: u8 = redis::cmd("HEXISTS")
            .arg("myhash3")
            .arg("field")
            .query(&mut con)?;

        assert!(exists == 0, "The field should be deleted after the overridden expiration of 2 seconds");

        Ok(())
    }

    #[test]
    fn test_expiremember_expiry_zero_deletes_immediately() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        // Set a field in a hash
        let _: () = redis::cmd("HSET")
            .arg("myhash5")
            .arg("field")
            .arg("value")
            .query(&mut con)?;

        // Initially set the expiration to a non-zero value
        let _: () = redis::cmd("EXPIREMEMBER")
            .arg("myhash5")
            .arg("field")
            .arg(5)
            .query(&mut con)?;

        std::thread::sleep(Duration::from_secs(1));

        // Reset the expiration to 0, effectively deleting the field
        let _: () = redis::cmd("EXPIREMEMBER")
            .arg("myhash5")
            .arg("field")
            .arg(0)
            .query(&mut con)?;

        // Check if the field is deleted immediately
        let exists: u8 = redis::cmd("HEXISTS")
            .arg("myhash5")
            .arg("field")
            .query(&mut con)?;

        assert!(exists == 0, "The field should be deleted immediately after setting expiration to 0");

        Ok(())
    }

    #[test]
    fn test_expiremember_set_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        // Add a member to a set using `redis::cmd`
        let _: () = redis::cmd("SADD")
            .arg("myset")
            .arg("member1")
            .query(&mut con)?;

        // Set expiration for the set member using custom `EXPIREMEMBER` command
        let _: () = redis::cmd("EXPIREMEMBER")
            .arg("myset")
            .arg("member1")
            .arg(2) // 2 seconds expiration
            .query(&mut con)?;
        std::thread::sleep(Duration::from_secs(1));

        let is_member: u8 = redis::cmd("SISMEMBER")
            .arg("myset")
            .arg("member1")
            .query(&mut con)?;
        assert!(is_member == 1, "The member should still exist in the set at this point in time");

        // Wait for more than 2 seconds
        std::thread::sleep(Duration::from_secs(2));

        // Check if the set member is deleted
        let is_member: u8 = redis::cmd("SISMEMBER")
            .arg("myset")
            .arg("member1")
            .query(&mut con)?;
        assert!(is_member == 0, "The set member should be deleted after expiration");

        Ok(())
    }

    #[test]
    fn test_expiremember_bulk_set_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        const NUM_MEMBERS: usize = 1000;
        const MAX_EXPIRATION: u64 = 10; // Max expiration time in seconds

        // Add members to a set with varying expiration times
        for i in 0..NUM_MEMBERS {
            let member = format!("member{}", i);
            let expire_in = (i % MAX_EXPIRATION as usize) as u64 + 1; // Expiration time between 1 to MAX_EXPIRATION seconds

            // Add the member
            let _: () = redis::cmd("SADD")
                .arg(format!("myset_bulk{}", expire_in / 2))
                .arg(&member)
                .query(&mut con)?;

            // Set the expiration
            let _: () = redis::cmd("EXPIREMEMBER")
                .arg(format!("myset_bulk{}", expire_in / 2))
                .arg(&member)
                .arg(expire_in)
                .query(&mut con)?;
        }

        let start = Instant::now();

        // Check for expirations every second and assert presence or absence of members
        for sec in 1..MAX_EXPIRATION {
            while start.elapsed().as_millis() < (sec * 1000 + 500) as u128  {
                std::thread::sleep(Duration::from_millis(50));
            }

            for i in 0..NUM_MEMBERS {
                let member = format!("member{}", i);
                let expiration_sec = (i % MAX_EXPIRATION as usize) as u64 + 1;
                let is_member: u8 = redis::cmd("SISMEMBER")
                    .arg(format!("myset_bulk{}", expiration_sec / 2))
                    .arg(&member)
                    .query(&mut con)?;

                if start.elapsed().as_millis() > 1000 * expiration_sec as u128 {
                    // Member should have expired
                    assert!(is_member == 0, "Member {} should have expired after {} seconds", member, sec);
                } else {
                    // Member should still exist
                    assert!(is_member == 1, "Member {} should still exist at {} seconds", member, sec);
                }
            }
        }

        Ok(())
    }

    #[test]
    fn test_expiremember_on_unset_key() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        // Attempt to set an expiration on a non-existent field in a hash
        let result: RedisResult<()> = redis::cmd("EXPIREMEMBER")
            .arg("nonexistent_hash")
            .arg("nonexistent_field")
            .arg(0) // Expiration set to 0 seconds
            .query(&mut con);

        // The command should complete without errors
        assert!(result.is_ok(), "Expiremember on a non-existent field should not cause an error");

        Ok(())
    }


    #[test]
    fn test_expiremember_zset_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        // Add a member to a sorted set using `redis::cmd`
        let _: () = redis::cmd("ZADD")
            .arg("myzset")
            .arg("1")
            .arg("member1")
            .query(&mut con)?;

        // Set expiration for the sorted set member using custom `EXPIREMEMBER` command
        let _: () = redis::cmd("EXPIREMEMBER")
            .arg("myzset")
            .arg("member1")
            .arg(2) // 2 seconds expiration
            .query(&mut con)?;
        std::thread::sleep(Duration::from_secs(1));

        let score: Option<f64> = redis::cmd("ZSCORE")
            .arg("myzset")
            .arg("member1")
            .query(&mut con)?;
        assert!(score.is_some(), "The member should still exist in the sorted set at this point in time");

        // Wait for more than 2 seconds
        std::thread::sleep(Duration::from_secs(2));

        // Check if the sorted set member is deleted
        let score: Option<f64> = redis::cmd("ZSCORE")
            .arg("myzset")
            .arg("member1")
            .query(&mut con)?;
        assert!(score.is_none(), "The sorted set member should be deleted after expiration");

        Ok(())
    }
    
    #[test]
    fn test_expiremember_bulk_zset_functionality() -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:34123/")?;
        let mut con = client.get_connection()?;

        const NUM_MEMBERS: usize = 1000;
        const MAX_EXPIRATION: u64 = 10; // Max expiration time in seconds

        // Add members to a sorted set with varying expiration times
        for i in 0..NUM_MEMBERS {
            let member = format!("member{}", i);
            let expire_in = (i % MAX_EXPIRATION as usize) as u64 + 1; // Expiration time between 1 to MAX_EXPIRATION seconds

            // Add the member
            let _: () = redis::cmd("ZADD")
                .arg(format!("myzset_bulk{}", expire_in / 2))
                .arg(i.to_string())
                .arg(&member)
                .query(&mut con)?;

            // Set the expiration
            let _: () = redis::cmd("EXPIREMEMBER")
                .arg(format!("myzset_bulk{}", expire_in / 2))
                .arg(&member)
                .arg(expire_in)
                .query(&mut con)?;
        }

        let start = Instant::now();

        // Check for expirations every second and assert presence or absence of members
        for sec in 1..MAX_EXPIRATION {
            while start.elapsed().as_millis() < (sec * 1000 + 500) as u128 {
                std::thread::sleep(Duration::from_millis(50));
            }

            for i in 0..NUM_MEMBERS {
                let member = format!("member{}", i);
                let expiration_sec = (i % MAX_EXPIRATION as usize) as u64 + 1;
                let score: Option<f64> = redis::cmd("ZSCORE")
                    .arg(format!("myzset_bulk{}", expiration_sec / 2))
                    .arg(&member)
                    .query(&mut con)?;

                if start.elapsed().as_millis() > 1000 * expiration_sec as u128 {
                    // Member should have expired
                    assert!(score.is_none(), "Member {} should have expired after {} seconds", member, sec);
                } else {
                    // Member should still exist
                    assert!(score.is_some(), "Member {} should still exist at {} seconds", member, sec);
                }
            }
        }

        Ok(())
    }
}
