# syntax=docker/dockerfile:1.4.3
ARG BASE
FROM ${BASE} as base

ENV DOCKER_CONFIG=/home/builder/.docker
USER root
RUN mkdir -p /twoliter/tools \
    && chown builder:builder /twoliter/tools \
    && chmod 755 /twoliter/tools
COPY --chown=builder:builder --chmod=755 ./files/Makefile.toml /twoliter/tools/Makefile.toml
USER builder
