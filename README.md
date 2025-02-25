# RPC proxy for Ethereum

Building lightweight reverse proxy like haproxy.
This repository is just started, so it is very early stage.

Health check mechanism should be different to basic Web 2 applications(backends)

In Web2, health check usually attempts accessing REST call and test get 200 OK response from the service.
But for blockchain client more like sophisticated mechanism required due to sync mechanism.

Even though clients are running well and serve its RPC(or API) endpoint,
it could not be working very well if clients ain't following the tip of chain data.

So, this project is building reverse proxy which is specified for blockchain client(for now Ethereum)

Feature should be added:
- Get various configs from CLI or env.


Features considering:
- Prometheus
- Opentracer
- Web based admin page
- REST API for managing internal status