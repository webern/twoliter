# syntax=docker/dockerfile:1.4.3
ARG BASE
FROM ${BASE} as base

USER root
RUN mkdir -p /local/.docker && chown -R builder:builder /local && chmod -R 777 /local
RUN echo '{"auths": {}}' > /local/.docker/config.json \
    && chown builder:builder /local/.docker/config.json \
    && chmod 777 /local/.docker/config.json
ENV DOCKER_CONFIG=/local/.docker

USER builder
COPY --chmod=755 buildsys /usr/local/bin
COPY --chmod=644 Makefile.toml /local
