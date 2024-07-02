#!/bin/sh
set -e

# start Router in background first since Router can sometimes break but that will not stop the deployment
# useful for debug environments where we will want to manually test and debug the router
echo "Starting Router"
nohup /dist/router --hr &

echo "Starting Helper"
nohup /dist/helper
