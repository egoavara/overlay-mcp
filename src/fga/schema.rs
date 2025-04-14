use serde_json::json;

pub fn schema() -> serde_json::Value {
    json!({
        "schema_version": "1.2",
        "type_definitions": [
          {
            "type": "ip",
            "relations": {
              "context": {
                "this": {}
              }
            },
            "metadata": {
              "relations": {
                "context": {
                  "directly_related_user_types": [
                    {
                      "type": "user",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                }
              },
              "module": "",
              "source_info": null
            }
          },
          {
            "type": "timetable",
            "relations": {
              "context": {
                "this": {}
              }
            },
            "metadata": {
              "relations": {
                "context": {
                  "directly_related_user_types": [
                    {
                      "type": "user",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                }
              },
              "module": "",
              "source_info": null
            }
          },
          {
            "type": "jwtclaim",
            "relations": {
              "context": {
                "this": {}
              }
            },
            "metadata": {
              "relations": {
                "context": {
                  "directly_related_user_types": [
                    {
                      "type": "user",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                }
              },
              "module": "",
              "source_info": null
            }
          },
          {
            "type": "user",
            "relations": {},
            "metadata": null
          },
          {
            "type": "api",
            "relations": {
              "allow": {
                "union": {
                  "child": [
                    {
                      "tupleToUserset": {
                        "tupleset": {
                          "object": "",
                          "relation": "allowed_ip"
                        },
                        "computedUserset": {
                          "object": "",
                          "relation": "context"
                        }
                      }
                    },
                    {
                      "tupleToUserset": {
                        "tupleset": {
                          "object": "",
                          "relation": "allowed_timetable"
                        },
                        "computedUserset": {
                          "object": "",
                          "relation": "context"
                        }
                      }
                    },
                    {
                      "tupleToUserset": {
                        "tupleset": {
                          "object": "",
                          "relation": "allowed_jwtclaim"
                        },
                        "computedUserset": {
                          "object": "",
                          "relation": "context"
                        }
                      }
                    }
                  ]
                }
              },
              "allowed_ip": {
                "this": {}
              },
              "allowed_jwtclaim": {
                "this": {}
              },
              "allowed_timetable": {
                "this": {}
              },
              "deny": {
                "union": {
                  "child": [
                    {
                      "tupleToUserset": {
                        "tupleset": {
                          "object": "",
                          "relation": "denyed_ip"
                        },
                        "computedUserset": {
                          "object": "",
                          "relation": "context"
                        }
                      }
                    },
                    {
                      "tupleToUserset": {
                        "tupleset": {
                          "object": "",
                          "relation": "denyed_timetable"
                        },
                        "computedUserset": {
                          "object": "",
                          "relation": "context"
                        }
                      }
                    },
                    {
                      "tupleToUserset": {
                        "tupleset": {
                          "object": "",
                          "relation": "denyed_jwtclaim"
                        },
                        "computedUserset": {
                          "object": "",
                          "relation": "context"
                        }
                      }
                    }
                  ]
                }
              },
              "denyed_ip": {
                "this": {}
              },
              "denyed_jwtclaim": {
                "this": {}
              },
              "denyed_timetable": {
                "this": {}
              }
            },
            "metadata": {
              "relations": {
                "allow": {
                  "directly_related_user_types": [],
                  "module": "",
                  "source_info": null
                },
                "allowed_ip": {
                  "directly_related_user_types": [
                    {
                      "type": "ip",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                },
                "allowed_jwtclaim": {
                  "directly_related_user_types": [
                    {
                      "type": "jwtclaim",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                },
                "allowed_timetable": {
                  "directly_related_user_types": [
                    {
                      "type": "timetable",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                },
                "deny": {
                  "directly_related_user_types": [],
                  "module": "",
                  "source_info": null
                },
                "denyed_ip": {
                  "directly_related_user_types": [
                    {
                      "type": "ip",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                },
                "denyed_jwtclaim": {
                  "directly_related_user_types": [
                    {
                      "type": "jwtclaim",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                },
                "denyed_timetable": {
                  "directly_related_user_types": [
                    {
                      "type": "timetable",
                      "condition": ""
                    }
                  ],
                  "module": "",
                  "source_info": null
                }
              },
              "module": "",
              "source_info": null
            }
          }
        ],
        "conditions": {}
      })
}
