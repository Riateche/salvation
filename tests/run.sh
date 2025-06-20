#!/bin/bash

set -ex

cd "$(dirname "$0")/.."

if [ "$1" == "--help" ]; then
    echo "Usage: run.sh [test_name]"
    exit 0
fi

if [[ -z "${GITHUB_ACTIONS}" ]]; then
    docker build \
        --tag salvation_tests \
        --file tests/Dockerfile \
        --build-arg BUILD_MODE \
        --progress plain \
        .
else
    echo "Skipping docker build in Github Actions"
fi

docker rm --force salvation_tests || true
docker run --name salvation_tests \
    --mount "type=bind,source=$PWD,target=/salvation" \
    --publish 25901:5901 --publish 26901:6901 \
    salvation_tests \
    salvation_tests test "$1"

# for i in {1..20}; do
#     sleep 0.3
#     echo Testing container status
#     docker exec salvation_tests xdotool click 1 || true
#     if docker exec salvation_tests xdotool getactivewindow; then
#        echo Container is ready
#        break
#     fi
#     if ! docker exec salvation_tests pidof xfwm4; then
#         echo xfwm4 is not running, starting xfwm4
#         docker exec --detach salvation_tests xfwm4
#     fi
# done
# if [ "$i" == "20" ]; then
#     2>&1 echo "Container check failed"
#     exit 1
# fi
# docker exec salvation_tests salvation_tests test "$1"
