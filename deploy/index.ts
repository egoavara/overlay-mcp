import { apps, core, apiextensions } from "@pulumi/kubernetes";
import { Config, output } from "@pulumi/pulumi";
import { RandomPassword } from "@pulumi/random";
import * as fs from "fs";
const config = new Config();

const ns = core.v1.Namespace.get("runtime", "runtime");
const raftSecret = new RandomPassword("raft-secret", {
    length: 32,
    special: true,
});
const overlayMcpConfig = new core.v1.Secret("overlay-mcp-secret", {
    metadata: {
        namespace: ns.metadata.name,
        name: "overlay-mcp-secret",
    },

    stringData: {
        "run.sh": output(fs.readFileSync("./run.sh", "utf-8")),
        "config.json": output({
            "application": {
                "log_filter": config.get("log_filter") || "info",
                "ip_extract": "RightmostXForwardedFor",
                "prometheus": true,
                "health_check": true,
                "apikey": [
                    {
                        "type": "header",
                        "name": "X-API-KEY"
                    },
                    {
                        "type": "query",
                        "name": "apikey"
                    }
                ]
            },
            "upstream": {
                "discovery": config.get("upstream")
            },
            "server": {
                "addr": "0.0.0.0:9090",
                "hostname": config.get("hostname"),
                "cluster": {
                    "type": "raft",
                    "secret": raftSecret.result,
                    "nodes": [
                        {
                            "id": 1,
                            "api": "overlay-mcp-0.overlay-mcp-headless.runtime.svc.cluster.local:8102",
                            "raft": "overlay-mcp-0.overlay-mcp-headless.runtime.svc.cluster.local:8103"
                        },
                        {
                            "id": 2,
                            "api": "overlay-mcp-1.overlay-mcp-headless.runtime.svc.cluster.local:8102",
                            "raft": "overlay-mcp-1.overlay-mcp-headless.runtime.svc.cluster.local:8103"
                        },
                        {
                            "id": 3,
                            "api": "overlay-mcp-2.overlay-mcp-headless.runtime.svc.cluster.local:8102",
                            "raft": "overlay-mcp-2.overlay-mcp-headless.runtime.svc.cluster.local:8103"
                        }
                    ]
                }
            },
            "idp": {
                "type": "oidc-discovery",
                "issuer": config.getSecret("oidcIssuer"),
                "client_id": config.getSecret("oidcClientId"),
                "client_secret": config.getSecret("oidcClientSecret"),
                "scopes": config.get("oidcScopes")?.split(",")
            },
            "authorizer": {
                "openfga": "http://openfga.runtime.svc.cluster.local:8080"
                // "jwt": {
                //     "allow_all": false,
                //     "group": {
                //         "whitelist": [
                //             "pg-users"
                //         ]
                //     },
                //     "ip": {
                //         "whitelist": "192.168.0.0/16",
                //         "blacklist": [
                //             "127.0.0.1",
                //             "192.168.220.0/24"
                //         ]
                //     }
                // }
            }
        }).apply(JSON.stringify)
    }
})

const appLabels = { app: "overlay-mcp" };
const deployment = build(config.get("image"));

const service = new core.v1.Service("overlay-mcp", {
    metadata: {
        namespace: ns.metadata.name,
        name: "overlay-mcp",
        labels: {
            "istio.io/use-waypoint": "waypoint"
        }
    },
    spec: {
        type: "LoadBalancer",
        ports: [
            {
                name: "http",
                protocol: "TCP",
                appProtocol: "http",
                port: 80,
                targetPort: 9090,
            }
        ],
        selector: appLabels,
    }
})

const serviceHeadless = new core.v1.Service("overlay-mcp-headless", {
    metadata: {
        namespace: ns.metadata.name,
        name: "overlay-mcp-headless",
    },
    spec: {
        type: "ClusterIP",
        clusterIP: "None",
        publishNotReadyAddresses: true,
        ports: [
            {
                name: "http",
                protocol: "TCP",
                appProtocol: "http",
                port: 80,
                targetPort: 9090,
            }
        ],
        selector: appLabels,
    }
})

const httpRoute = new apiextensions.CustomResource("overlay-mcp-route", {
    apiVersion: "gateway.networking.k8s.io/v1",
    kind: "HTTPRoute",
    metadata: {
        name: "overlay-mcp-route",
        namespace: ns.metadata.name,
    },
    spec: {
        hostnames: [
            "mcp-db.egoavara.net",
        ],
        parentRefs: [{
            kind: "Gateway",
            name: config.getSecret("defaultGatewayName"),
            namespace: config.getSecret("defaultGatewayNamespace"),
        }],
        rules: [
            {
                matches: [
                    {
                        path: {
                            type: "PathPrefix",
                            value: "/",
                        },
                    }
                ],
                backendRefs: [{
                    kind: "Service",
                    name: service.metadata.name,
                    namespace: service.metadata.namespace,
                    port: 80,
                }]
            }
        ],
    }
})

function build(image: string | undefined) {
    if (image) {
        return buildImage(image);
    }
    return buildFromSource();
}
function buildImage(image: string) {
    return new apps.v1.StatefulSet("overlay-mcp", {
        metadata: {
            namespace: ns.metadata.name,
            name: "overlay-mcp",
        },
        spec: {
            serviceName: "overlay-mcp-headless",
            replicas: 3,
            selector: {
                matchLabels: appLabels
            },
            template: {
                metadata: {
                    labels: appLabels
                },
                spec: {
                    volumes: [
                        {
                            name: "overlay-mcp",
                            secret: {
                                secretName: overlayMcpConfig.metadata.name,
                            }
                        }
                    ],
                    containers: [
                        {
                            name: "overlay-mcp",
                            image: image,
                            args: [
                                "run",
                                "-c",
                                "/data/overlay-mcp/config.json"
                            ],
                            env: [
                                {
                                    name: "OVERLAY_MCP_RAFT_INDEX",
                                    valueFrom: {
                                        fieldRef: {
                                            fieldPath: "metadata.labels['apps.kubernetes.io/pod-index']"
                                        }
                                    }
                                }
                            ],
                            ports: [
                                {
                                    containerPort: 9090,
                                    protocol: "TCP",
                                }
                            ],
                            readinessProbe: {
                                httpGet: {
                                    path: "/.meta/health",
                                    port: 9090,
                                },
                                initialDelaySeconds: 1,
                                periodSeconds: 5,
                                failureThreshold: 20,
                                successThreshold: 1,
                                timeoutSeconds: 10,
                            },
                            livenessProbe: {
                                httpGet: {
                                    path: "/.meta/health",
                                    port: 9090,
                                },
                                initialDelaySeconds: 1,
                                periodSeconds: 5,
                                failureThreshold: 20,
                                successThreshold: 1,
                                timeoutSeconds: 10,
                            },
                            volumeMounts: [
                                {
                                    name: "overlay-mcp",
                                    mountPath: "/data/overlay-mcp",
                                }
                            ]
                        }
                    ]
                }
            }
        }
    })
}
function buildFromSource() {
    return new apps.v1.StatefulSet("overlay-mcp", {
        metadata: {
            namespace: ns.metadata.name,
            name: "overlay-mcp",
        },
        spec: {
            serviceName: "overlay-mcp-headless",
            selector: {
                matchLabels: appLabels
            },
            template: {
                metadata: {
                    labels: appLabels
                },
                spec: {
                    volumes: [
                        {
                            name: "overlay-mcp",
                            secret: {
                                secretName: overlayMcpConfig.metadata.name,
                            }
                        },
                        {
                            name: "run-sh",
                            secret: {
                                secretName: overlayMcpConfig.metadata.name,
                                items: [
                                    {
                                        key: "run.sh",
                                        path: "run.sh",
                                    }
                                ],
                                defaultMode: 0o555,
                            },
                        },
                        {
                            name: "git-sync",
                            emptyDir: {}
                        }
                    ],
                    containers: [
                        {
                            name: "overlay-mcp",
                            image: "rust:1.86-bookworm",
                            command: [
                                "/bin/bash",
                                "-c",
                                "/data/run-sh/run.sh",
                            ],
                            env: [
                                {
                                    name: "OVERLAY_MCP_RAFT_INDEX",
                                    valueFrom: {
                                        fieldRef: {
                                            fieldPath: "metadata.labels['apps.kubernetes.io/pod-index']"
                                        }
                                    }
                                }
                            ],
                            ports: [
                                {
                                    containerPort: 9090,
                                    protocol: "TCP",
                                }
                            ],
                            readinessProbe: {
                                httpGet: {
                                    path: "/.meta/health",
                                    port: 9090,
                                },
                                initialDelaySeconds: 30,
                                periodSeconds: 5,
                                failureThreshold: 20,
                                successThreshold: 1,
                                timeoutSeconds: 10,
                            },
                            livenessProbe: {
                                httpGet: {
                                    path: "/.meta/health",
                                    port: 9090,
                                },
                                initialDelaySeconds: 30,
                                periodSeconds: 5,
                                failureThreshold: 20,
                                successThreshold: 1,
                                timeoutSeconds: 10,
                            },
                            volumeMounts: [
                                {
                                    name: "overlay-mcp",
                                    mountPath: "/data/overlay-mcp",
                                },
                                {
                                    name: "run-sh",
                                    mountPath: "/data/run-sh",
                                },
                                {
                                    name: "git-sync",
                                    mountPath: "/data/git-sync",
                                }
                            ]
                        },
                        {
                            name: "git-sync",
                            image: "registry.k8s.io/git-sync/git-sync:v4.4.0",
                            env: [
                                {
                                    name: "GITSYNC_REPO",
                                    value: "https://github.com/egoavara/overlay-mcp.git",
                                },
                                {
                                    name: "GITSYNC_BRANCH",
                                    value: "main",
                                },
                                {
                                    name: "GITSYNC_ROOT",
                                    value: "/data/git-sync",
                                },
                                {
                                    name: "GITSYNC_PERIOD",
                                    value: "1s",
                                }
                            ],
                            volumeMounts: [
                                {
                                    name: "git-sync",
                                    mountPath: "/data/git-sync",
                                }
                            ]
                        },
                    ]
                }
            }
        }
    })
}