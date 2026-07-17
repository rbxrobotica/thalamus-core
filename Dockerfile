# thalamus-server container (master plan §4).
# Institutional build: durable audit (postgres) + LiteLLM backend adapter.
FROM docker.io/library/rust:1.92-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p thalamus-server --features postgres,litellm \
    && cargo build --release -p thalamus-postgres-adapter --bin thalamus-migrate

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home thalamus
COPY --from=build /src/target/release/thalamus-server /usr/local/bin/thalamus-server
COPY --from=build /src/target/release/thalamus-migrate /usr/local/bin/thalamus-migrate
USER 10001
EXPOSE 8080
ENTRYPOINT ["thalamus-server"]
# Policy config is mounted by the deployment (ConfigMap).
CMD ["/etc/thalamus/policy.json"]
