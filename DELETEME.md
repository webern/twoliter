```shell
docker \
"run" \
"--mount" \
"type=bind,source=/home/ANT.AMAZON.COM/brigmatt/repos/twoliter/.ignore/hack/bottlerocket,target=/home/ANT.AMAZON.COM/brigmatt/repos/twoliter/.ignore/hack/bottlerocket" \
"--mount" \
"type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock" \
"--name" \
"twoliter-exec" \
"--rm" \
"--user" \
"632230848" \
"twoliter:latest" \
"cargo" \
"make" \
"--disable-check-for-updates" \
"--makefile" \
"/local/Makefile.toml" \
"--cwd" \
"/home/ANT.AMAZON.COM/brigmatt/repos/twoliter/.ignore/hack/bottlerocket" \
"--verbose" \
"-e" \
"BUILDSYS_ROOT_DIR=/home/ANT.AMAZON.COM/brigmatt/repos/twoliter/.ignore/hack/bottlerocket"
```

```shell
docker \
"run" \
"-it" \
"--mount" \
"type=bind,source=/home/ANT.AMAZON.COM/brigmatt/repos/twoliter/.ignore/hack/bottlerocket,target=/home/ANT.AMAZON.COM/brigmatt/repos/twoliter/.ignore/hack/bottlerocket" \
"--mount" \
"type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock" \
"--name" \
"twoliter-exec" \
"--rm" \
"--user" \
"632230848" \
"twoliter:latest" \
"bash"
```