# syntax=docker/dockerfile:1.2
FROM scalesocket/scalesocket:latest

RUN apk upgrade --no-cache && \
    apk add --no-cache gcompat libgcc

WORKDIR /app

CMD scalesocket --addr 0.0.0.0:8000 \
    --staticdir /var/www/public/ \
    --null \
    md -- --interactive --log=html --output stdout
