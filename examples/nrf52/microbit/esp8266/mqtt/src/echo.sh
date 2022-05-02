#!/bin/bash
APP=$1;
while true
do
  echo "APP "$APP
  message=$(drg stream -n 1 $APP)
    echo "Received message: "$message;
  if [ "$message" = "" ]; then
    continue;
  fi

  message=$(echo $message | jq -r '.data_base64' | base64 -d | tr "'" '"');
  echo "'"$message"'";
  echo $message | http POST https://api.sandbox.drogue.cloud/api/command/v1alpha1/apps/$APP/devices/device1 command==set-temp "Authorization:Bearer $(drg whoami -t)"
done
