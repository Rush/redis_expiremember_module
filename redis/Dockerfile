FROM redis:latest
COPY ./target/release/libredis_expiremember_module.so /libredis_expiremember_module.so

CMD [ "redis-server", "--loadmodule", "/libredis_expiremember_module.so", "/usr/local/etc/redis/redis.conf" ]


