#!/bin/sh
set -e

echo "Starting Helper"
nohup /dist/helper &

echo "Starting Router"
nohup /dist/router --hr
