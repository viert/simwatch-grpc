FROM ubuntu:jammy
RUN apt update && apt install -y ca-certificates
ADD target/release/simwatch-grpc /usr/bin/simwatch-grpc
RUN mkdir /etc/simwatch
ADD simwatch-grpc.toml /etc/simwatch/simwatch-grpc.toml
CMD [ "/usr/bin/simwatch-grpc", "-c", "/etc/simwatch/simwatch-grpc.toml" ]
