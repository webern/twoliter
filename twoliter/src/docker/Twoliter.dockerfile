# syntax=docker/dockerfile:1.4.3
ARG BASE
FROM ${BASE} as base

ENV TWOLITER_DIR=/twoliter
ENV TWOLITER_TOOLS="${TWOLITER_DIR}/tools"
ENV DOCKER_CONFIG="${TWOLITER_DIR}/.docker"
USER root
RUN mkdir -p "${TWOLITER_DIR}" "${TWOLITER_TOOLS}" "${DOCKER_CONFIG}" \
    && chown builder:builder "${TWOLITER_DIR}" "${TWOLITER_TOOLS}" "${DOCKER_CONFIG}" \
    && chmod -R 777 "${TWOLITER_DIR}" \
    && chmod -R 777 "${DOCKER_CONFIG}" \
    && chmod -R 755 "${TWOLITER_TOOLS}"
COPY --chown=builder:builder --chmod=755 ./files/Makefile.toml /twoliter/tools/Makefile.toml
USER builder
