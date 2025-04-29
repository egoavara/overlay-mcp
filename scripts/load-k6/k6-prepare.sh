#! /bin/bash

go install go.k6.io/xk6/cmd/xk6@latest

xk6 build \
  --with github.com/grafana/xk6-output-influxdb \
  --with github.com/phymbert/xk6-sse \
  --output k6