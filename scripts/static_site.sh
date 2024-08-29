#!/bin/bash

echo $ARTIFACT_LOCATION

unzip $ARTIFACT_LOCATION -d /var/www/$DEPLOYMENT_NAME

echo "Deployment has completed"