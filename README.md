# HTCache - Simple and fast cache with HTTP interface

## Starting the service

```sh
htcache -a 0.0.0.0 -p 9000
```

## 

This demo application uses the following techniques and libraries:

 * "Warp" for creating an lightweight HTTP server with REST interface
 * "Clap" for creating a nice command line interface
 * "Tokio" to run the garbage collector asynchronous
 * "Serde" and "Serde JSON" to serialize and deserialize the cache