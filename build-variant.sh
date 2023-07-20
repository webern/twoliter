#!/usr/bin/env bash
set -e

cd $REPOS/twoliter
SHA=$(git rev-parse HEAD)

cd $REPOS/bottlerocket

source $REPOS/notes/commands/build-k8s.env

rm -f "${BOTTLEROCKET}/Infra.toml"
ln -s "${INFRA_TOML}" "${BOTTLEROCKET}/Infra.toml"

cd "${BOTTLEROCKET}" && cargo make \
  -e TWOLITER_REV=$SHA \
  -e PUBLISH_INFRA_CONFIG_PATH="${INFRA_TOML}" \
  -e BUILDSYS_VARIANT=$MY_VARIANT \
  -e BUILDSYS_ARCH=$MY_ARCH \
  -e BUILDSYS_UPSTREAM_SOURCE_FALLBACK=true \
  build

SHORT_SHA=$(git describe --always --dirty --exclude '*' || echo 00000000)
AMI_NAME="${MY_VARIANT}-${MY_ARCH}-${SHORT_SHA}-$(uuidgen)"
cd "${BOTTLEROCKET}" && cargo make \
  -e TWOLITER_REV=$SHA \
  -e PUBLISH_INFRA_CONFIG_PATH="${INFRA_TOML}" \
  -e BUILDSYS_VARIANT=$MY_VARIANT \
  -e BUILDSYS_ARCH=$MY_ARCH \
  -e PUBLISH_AMI_NAME="${AMI_NAME}" \
  ami
AMIS_JSON="${BOTTLEROCKET}/build/images/${MY_ARCH}-${MY_VARIANT}/latest/bottlerocket-${MY_VARIANT}-${MY_ARCH}-amis.json"
AMI_ID=$(cat $AMIS_JSON | jq -r --arg MY_REGION "${MY_REGION}" '.[$MY_REGION].id')

cargo make \
  -e TWOLITER_REV=$SHA \
  -e PUBLISH_INFRA_CONFIG_PATH="${INFRA_TOML}" \
  -e BUILDSYS_VARIANT=$MY_VARIANT \
  -e BUILDSYS_ARCH=$MY_ARCH \
  clean-repos

cd "${BOTTLEROCKET}" && cargo make \
  -e TWOLITER_REV=$SHA \
  -e PUBLISH_INFRA_CONFIG_PATH="${INFRA_TOML}" \
  -e BUILDSYS_VARIANT=$MY_VARIANT \
  -e BUILDSYS_ARCH=$MY_ARCH \
  repo
