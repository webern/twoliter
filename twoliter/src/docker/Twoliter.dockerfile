# syntax=docker/dockerfile:1.4.3
ARG BASE
FROM ${BASE} as base

RUN mkdir -p /home/builder/tools
COPY --chown=builder:builder --chmod=755 ./files/ /home/builder/tools/
