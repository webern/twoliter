docker run --rm -it \
  --mount type=bind,source=/tmp/mountme,target=/tmp/mountme \
  twoliter:latest \
  bash
