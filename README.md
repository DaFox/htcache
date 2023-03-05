# HTCache - Simple and fast cache with HTTP interface

[![Rust](https://github.com/DaFox/htcache/actions/workflows/rust.yml/badge.svg)](https://github.com/DaFox/htcache/actions/workflows/rust.yml)

## Starting the service

```sh
htcache -a 0.0.0.0 -p 9000
```

## Usage

### Write data to the cache

```
PUT /<cache-key>
Content-Type: <content-type>
X-TTL: <ttl>
```

```sh
curl -XPUT http://localhost:3030/test --header "Content-Type: text/plain" --header "X-TTL: 120" --data-binary="hello world"
```

### Read data from the cache

```
GET /<cache-key>
```

```sh
curl -XGET http://localhost:3030/test
```

## About this demo

This demo application uses the following techniques and libraries:

 * "Warp" for creating an lightweight HTTP server with REST interface
 * "Clap" for creating a nice command line interface
 * "Tokio" to run the garbage collector asynchronous
 * "Serde" and "Serde JSON" to serialize and deserialize the cache
