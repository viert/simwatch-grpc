FROM ubuntu:jammy
ADD target/release/simwatch-grpc /usr/bin/simwatch-grpc
RUN mkdir /etc/simwatch
ADD simwatch-grpc.toml /etc/simwatch/simwatch-grpc.toml
CMD [ "/usr/bin/simwatch-grpc", "-c", "/etc/simwatch/simwatch-grpc.toml" ]
