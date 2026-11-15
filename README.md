# Benjamin Larkey resume site

## Run locally

Copy `.env.example` to `.env`, then replace both placeholder values. Generate a session key with `openssl rand -base64 48` and paste the result into `.env`:

```sh
cp .env.example .env
openssl rand -base64 48
```

Start the app with:

```sh
cargo run
```

Open <http://127.0.0.1:8080>. The blog is at `/blog`; sign in at `/admin` to publish. Posts are stored in `data/blog_posts.json` and sorted by their date.

Keep `.env` private; it is excluded from version control. For a deployed site, set both values in the hosting provider's secret configuration and serve the app over HTTPS. The session cookie is marked `Secure` in release builds.
