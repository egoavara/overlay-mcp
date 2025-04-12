import { apps, core, apiextensions } from "@pulumi/kubernetes";
import { Config, output } from "@pulumi/pulumi";
import * as fs from "fs";
const config = new Config();

const ns = core.v1.Namespace.get("runtime", "runtime");

const overlayMcpConfig = new core.v1.ConfigMap("overlay-mcp-config", {
    metadata: {
        namespace: ns.metadata.name,
        name: "overlay-mcp-config",
    },

    data: {
        "run.sh": output(fs.readFileSync("./run.sh", "utf-8")),
        "config.json": output({
            "server": {
                "addr": "0.0.0.0:9090",
                "hostname": config.get("hostname"),
                "upstream": config.get("upstream")
            },
            "oidc": {
                "issuer": config.getSecret("oidcIssuer"),
                "client_id": config.getSecret("oidcClientId"),
                "client_secret": config.getSecret("oidcClientSecret"),
                "scopes": config.get("oidcScopes")?.split(",")
            },
        }).apply(JSON.stringify)
    }
})

const appLabels = { app: "overlay-mcp" };
const deployment = new apps.v1.Deployment("overlay-mcp", {
    metadata: {
        namespace: ns.metadata.name,
        name: "overlay-mcp",
    },
    spec: {
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
                        configMap: {
                            name: overlayMcpConfig.metadata.name,
                        }
                    },
                    {
                        name: "run-sh",
                        configMap: {
                            name: overlayMcpConfig.metadata.name,
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
                            "/data/run-sh/run.sh"
                        ],
                        ports: [
                            {
                                containerPort: 9090,
                                protocol: "TCP",
                            }
                        ],
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
                                value: "30s",
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

const serviceProxy = new core.v1.Service("mcp-db-proxy", {
    metadata: {
        namespace: ns.metadata.name,
        name: "overlay-mcp-proxy",
    },
    spec: {
        type: "LoadBalancer",
        ports: [
            {
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
                backendRefs: [{
                    kind: "Service",
                    name: serviceProxy.metadata.name,
                    namespace: serviceProxy.metadata.namespace,
                    port: 80,
                }]
            }
        ],
    }
})