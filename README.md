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

Open <http://127.0.0.1:8080>. The blog is at `/blog`; sign in at `/admin` to publish. Each post is stored as `data/<slug>.md` with title, date, and slug metadata, followed by its Markdown body. Existing `blog_posts.json` data is migrated to Markdown files at startup; the JSON file is retained as a backup. Supported formatting includes headings, bold and italic text, inline and fenced code, links, lists, and blockquotes. Each post has its own `/blog/<slug>` page, and the blog archive can search post titles and content.

Keep `.env` private; it is excluded from version control. For a deployed site, set both values in the hosting provider's secret configuration and serve the app over HTTPS. The session cookie is marked `Secure` in release builds.

## Deploy on Railway

Railway detects the root `Dockerfile` and builds the site as a container. Add `BLOG_ADMIN_PASSWORD` and `BLOG_SESSION_KEY` as Railway service variables; generate the session key with `openssl rand -base64 48`. Railway provides `PORT`, which the app listens on.

Set the service's healthcheck path to `/health`. It returns HTTP 200 with the JSON value `true` when the app is running.

Blog posts are stored as Markdown files in `data/`. Attach a Railway volume at `/app/data` if posts created in the admin need to survive container replacements and deployments.
