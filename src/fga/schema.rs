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
                    "type": "user"
                  }
                ]
              }
            }
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
                    "type": "user"
                  }
                ]
              }
            }
          }
        },
        {
          "type": "apikey",
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
                    "type": "user"
                  }
                ]
              }
            }
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
                    "type": "user"
                  }
                ]
              }
            }
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
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "allowed_ip"
                      }
                    }
                  },
                  {
                    "tupleToUserset": {
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "allowed_timetable"
                      }
                    }
                  },
                  {
                    "tupleToUserset": {
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "allowed_jwtclaim"
                      }
                    }
                  },
                  {
                    "tupleToUserset": {
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "allowed_apikey"
                      }
                    }
                  }
                ]
              }
            },
            "allowed_ip": {
              "this": {}
            },
            "allowed_apikey": {
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
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "denyed_ip"
                      }
                    }
                  },
                  {
                    "tupleToUserset": {
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "denyed_timetable"
                      }
                    }
                  },
                  {
                    "tupleToUserset": {
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "denyed_jwtclaim"
                      }
                    }
                  },
                  {
                    "tupleToUserset": {
                      "computedUserset": {
                        "relation": "context"
                      },
                      "tupleset": {
                        "relation": "denyed_apikey"
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
            "denyed_apikey": {
              "this": {}
            },
            "denyed_timetable": {
              "this": {}
            }
          },
          "metadata": {
            "relations": {
              "allow": {
                "directly_related_user_types": []
              },
              "allowed_ip": {
                "directly_related_user_types": [
                  {
                    "type": "ip"
                  }
                ]
              },
              "allowed_apikey": {
                "directly_related_user_types": [
                  {
                    "type": "apikey"
                  }
                ]
              },
              "allowed_jwtclaim": {
                "directly_related_user_types": [
                  {
                    "type": "jwtclaim"
                  }
                ]
              },
              "allowed_timetable": {
                "directly_related_user_types": [
                  {
                    "type": "timetable"
                  }
                ]
              },
              "deny": {
                "directly_related_user_types": []
              },
              "denyed_ip": {
                "directly_related_user_types": [
                  {
                    "type": "ip"
                  }
                ]
              },
              "denyed_jwtclaim": {
                "directly_related_user_types": [
                  {
                    "type": "jwtclaim"
                  }
                ]
              },
              "denyed_apikey": {
                "directly_related_user_types": [
                  {
                    "type": "apikey"
                  }
                ]
              },
              "denyed_timetable": {
                "directly_related_user_types": [
                  {
                    "type": "timetable"
                  }
                ]
              }
            }
          }
        }
      ]
    })
}
