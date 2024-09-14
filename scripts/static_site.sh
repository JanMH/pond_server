#!/usr/bin/env bash
set -e

echo $ARTIFACT_LOCATION

rm -rf /var/www/$DEPLOYMENT_NAME || true
unzip $ARTIFACT_LOCATION -d /var/www/$DEPLOYMENT_NAME