#!/usr/bin/env bash

set -e

script_dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
root_dir=$( cd "$script_dir/../../.." && pwd )
logs_dir="${root_dir}/.ignore/logs"

echo "script_dir: ${script_dir}"
echo "logs_dir: ${logs_dir}"

mkdir -p "${logs_dir}"
cd "${script_dir}"

export BUILDSYS_ROOT_DIR="${script_dir}"
export BUILDSYS_VARIANT="fake-dev"
export BUILDSYS_ARCH="x86_64"

cargo run \
   --release \
   --package twoliter \
   --manifest-path "${script_dir}/../../../Cargo.toml" \
   -- \
      make build-variant \
        --cargo-home "${script_dir}/.cargo" \
      2>&1 | tee "${logs_dir}/bottlerocket-like-test.log"
