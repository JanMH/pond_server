# Pond Server

This is the server component of the pond deployment tool. Pond supports uploading artifacts and deploying them through so called "deployers". Currently, only the `static-site` deployment type is supported.

## Prerequisites

To build and install the server you will need to have rust and cargo installed. You can install them using [rustup](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Additionally, pond relies on [nginx](https://nginx.org/) and [certbot](https://certbot.eff.org/) to serve deployed artifacts and enable ssh for the pond server.

## Installation

To build and install the server, you need to have [Rust](https://www.rust-lang.org/tools/install) installed. You can then build and install the server using `cargo`:

1. Clone the repository:

    ```sh
    git clone https://github.com/JanMH/pond_server
    cd pond_server
    ```

2. Build and install the server:
    ```sh
    export POND_CONFIG_DEFAULT_PATH=/etc/pond/pond.toml
    cargo build --release
    sudo cp target/release/pond_server /usr/local/bin
    ```
3. Create necessary directories and copy required files:

    ```sh
    sudo mkdir /etc/pond
    sudo cp -r scripts /etc/pond/
    ```

4. Create a configuration file at `/etc/pond/pond.toml`:

    ```toml
    [default]
    address = "127.0.0.1"
    scripts_location = "/etc/pond/scripts"
    root_domain_name = "your-domain.com"
    log_level = "normal"
    access_token = <Put a random access token here>
    
    [default.limits]
    file = "1GB"
    ```

5. Create a systemd service file at `/etc/systemd/system/pond_server.service`:

    ```ini
    [Unit]
    Description=Pond Server
    After=network.target

    [Service]
    Type=simple
    User=root
    Group=root
    ExecStart=/usr/local/bin/pond_server
    Restart=always

    [Install]
    WantedBy=multi-user.target
    ```
6. Start and enable the service:

    ```sh
    sudo systemctl start pond_server
    sudo systemctl enable pond_server
    ```
7. Configure nginx and certbot to serve the deployed artifacts and enable ssh for the pond server.

   Create the nginx configuration file at `/etc/nginx/sites-available/pond.your-domain.com`:
    
    ```nginx
    server {
        server_name pond.your-domain.com;
        client_max_body_size 1G;
        location / {
            proxy_pass http://localhost:8000;
        }
        proxy_buffering off;
    }
    ```
    Create a symlink to the sites-enabled directory:

    ```sh
    sudo ln -s /etc/nginx/sites-available/pond.your-domain.com /etc/nginx/sites-enabled/
    ```
    Create a certificate for the domain using certbot:

    ```sh
    sudo certbot --nginx -d pond.your-domain.com
    ```