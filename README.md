# Simwatch grpc service

This is a next generation Simwatch service based on GRPC.
The main purpose of the service is fetching vatsim real-time data from the API and streaming changes within a given bounding box.

> _The previous generation of Simwatch service was web-based and built on top of a great rocket.rs library. Since the stable version of rocket.rs does not support websockets natively, the next simwatch API is going to be implemented in python using FastAPI, but to keep things fast, the primary functionality stays in the same rust codebase, the python API is supposed to use this service via GRPC and work more like a GRPC <-> Websockets proxy._

```
  rpc MapUpdates(stream MapUpdatesRequest) returns (stream Update);
```

`MapUpdatesRequest` allows you to set the bounding box to watch changes in, enable/disable weather information in airports and to configure a pilots filter in a generic expression dsl, e.g.

```
callsign =~ "^BAW" and altitude > 3000
```

There's also unary GRPC calls to fetch airports by a code and pilots by a callsign.

### Python bindings generation

To setup python bindings use the following command while in the rust project root folder.

```
python3.11 -m grpc_tools.protoc --python_out=$PYTHON_PROJECT_DIR/proto --grpc_python_out=$PYTHON_PROJECT_DIR/proto --proto_path=proto camden.proto
```
