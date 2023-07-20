#!/usr/bin/env bash

cd $REPOS/twoliter
SHA=$(git rev-parse HEAD)

cd $REPOS/bottlerocket
cargo make -e TWOLITER_REV=$SHA build-variant
