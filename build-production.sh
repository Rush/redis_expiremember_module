#!/bin/bash
docker build -t builder .
docker run --rm -v $(pwd):/usr/src/myapp -w /usr/src/myapp builder cargo build --release

