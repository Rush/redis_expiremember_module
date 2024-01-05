# Use the official Rust image as a base
FROM rust:1.75-bullseye

WORKDIR /usr/src/myapp

RUN apt update && apt install -y clang

# Set default command (optional)
CMD ["bash"]


