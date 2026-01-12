
## Rough notes

```bash
sudo ip link add test0 type dummy
sudo ip addr add 10.1.1.1/24 dev test0
```

`config.corn` ->

```corn
{
  log_level = "info"
  iface = "test0"

  ddns = {
      provider = {
          name = "digitalocean"
          key = $env_DIGITALOCEAN_API_KEY
    }

    domain = "haltcondition.net"
    host = "test"
  }
}
```


`cargo run -- -c config.corn`
