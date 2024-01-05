#!/bin/bash
docker build -t rushpl/redis-expiremember -f redis/Dockerfile .
version=$(grep "^version" Cargo.toml | cut -d '"' -f 2)
docker tag rushpl/redis-expiremember rushpl/redis-expiremember:${version}
