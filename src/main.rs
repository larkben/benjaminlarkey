use std::{env, fs, io, path::{Path, PathBuf}, sync::Mutex};

use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    App, HttpResponse, HttpServer, Responder, cookie::Key, get, http::header, post, web,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Post {
    #[serde(default)]
    slug: String,
    title: String,
    date: String,
    body: String,
}

struct AppState {
    posts: Mutex<Vec<Post>>,
    posts_dir: PathBuf,
    admin_password: String,
}

#[derive(Deserialize)]
struct LoginForm {
    password: String,
}

#[derive(Deserialize)]
struct NewPostForm {
    title: String,
    date: String,
    body: String,
}

#[get("/")]
async fn home(state: web::Data<AppState>) -> impl Responder {
    let posts = read_posts(&state);
    let feed = if posts.is_empty() {
        "<p class=\"empty-posts\">Awaiting the first transmission...</p>".to_owned()
    } else {
        let items = posts.iter().take(3).map(|post| {
            let title = escape_html(&post.title);
            let date = escape_html(&post.date);
            let preview = escape_html(&preview(&post.body, 150));
            format!("<article class=\"feed-post\"><a href=\"/blog/{}\"><span class=\"terminal-date\">[{date}]</span><strong>{title}</strong><span class=\"terminal-preview\">{preview}</span></a></article>", escape_html(&post.slug))
        }).collect::<String>();
        items
    };
    let all_link = if posts.is_empty() { String::new() } else { "<a class=\"all-posts\" href=\"/blog\">OPEN FULL ARCHIVE →</a>".to_owned() };
    let blog_feed_html = format!("{feed}{all_link}");
    let html = include_str!("index.html").replace("<!-- BLOG_FEED -->", &blog_feed_html);
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(html)
}

#[get("/projects")]
async fn projects() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("projects.html"))
}

#[get("/theme.js")]
async fn theme_script() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/javascript; charset=utf-8")
        .body(include_str!("theme.js"))
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(true)
}

#[get("/images/fhir-project.svg")]
async fn fhir_image() -> impl Responder {
    HttpResponse::Ok().content_type("image/svg+xml").body(include_str!("images/fhir-project.svg"))
}

#[get("/images/hl7-project.svg")]
async fn hl7_image() -> impl Responder {
    HttpResponse::Ok().content_type("image/svg+xml").body(include_str!("images/hl7-project.svg"))
}

#[get("/images/interface-project.svg")]
async fn interface_image() -> impl Responder {
    HttpResponse::Ok().content_type("image/svg+xml").body(include_str!("images/interface-project.svg"))
}

#[get("/blog")]
async fn blog() -> impl Responder {
    let posts = read_posts_from_file();
    let rendered_posts = if posts.is_empty() {
        "<p>No posts yet. Check back soon.</p>".to_owned()
    } else {
        posts.iter().map(render_post_card).collect::<String>()
    };
    let html = include_str!("blog.html").replace("{{POSTS}}", &rendered_posts);
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(html)
}

#[get("/blog/{slug}")]
async fn blog_post(path: web::Path<String>) -> impl Responder {
    let slug = path.into_inner();
    let posts = read_posts_from_file();
    let Some(post) = posts.iter().find(|post| post.slug == slug) else {
        return HttpResponse::NotFound().content_type("text/html; charset=utf-8")
            .body("<!doctype html><title>Post not found</title><p>That post could not be found. <a href='/blog'>Return to the blog</a>.</p>");
    };
    let body = render_markdown(&post.body);
    let html = include_str!("blog_post.html")
        .replace("{{TITLE}}", &escape_html(&post.title))
        .replace("{{DATE}}", &escape_html(&post.date))
        .replace("{{BODY}}", &body);
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(html)
}

#[get("/admin")]
async fn admin(session: Session, state: web::Data<AppState>) -> impl Responder {
    if !is_admin(&session) {
        return redirect("/admin/login");
    }
    let posts = read_posts(&state);
    let post_list = if posts.is_empty() {
        "<p class=\"muted\">No posts yet.</p>".to_owned()
    } else {
        posts.iter().map(|post| format!(
            "<li><strong>{}</strong><span>{}</span></li>",
            escape_html(&post.title), escape_html(&post.date)
        )).collect::<String>()
    };
    let html = admin_page(&format!(r#"
      <p class="eyebrow">Signed in as Benjamin</p>
      <h1>Write a post</h1>
      <form method="post" action="/admin/posts">
        <label>Title<input name="title" maxlength="120" required /></label>
        <label>Date<input name="date" type="date" required /></label>
      <label>Post <span class="muted">Markdown supported: # headings, **bold**, *italic*, `code`, [links](https://…), - lists, and &gt; quotes.</span><textarea name="body" rows="12" required></textarea></label>
        <button type="submit">Publish post</button>
      </form>
      <h2>Published posts</h2><ul class="post-list">{post_list}</ul>
      <form class="logout" method="post" action="/admin/logout"><button class="quiet" type="submit">Log out</button></form>
    "#));
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(html)
}

#[get("/admin/login")]
async fn login_page(session: Session) -> impl Responder {
    if is_admin(&session) {
        return redirect("/admin");
    }
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(admin_page(r#"
      <p class="eyebrow">Private area</p>
      <h1>Admin login</h1>
      <p class="muted">Sign in to write and publish blog posts.</p>
      <form method="post" action="/admin/login">
        <label>Password<input name="password" type="password" autocomplete="current-password" required /></label>
        <button type="submit">Log in</button>
      </form>
    "#))
}

#[post("/admin/login")]
async fn login(form: web::Form<LoginForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if form.password == state.admin_password {
        let _ = session.insert("is_admin", true);
        redirect("/admin")
    } else {
        HttpResponse::Unauthorized().content_type("text/html; charset=utf-8").body(admin_page(r#"
          <p class="eyebrow">Private area</p><h1>Admin login</h1>
          <p class="error">That password did not match. Please try again.</p>
          <form method="post" action="/admin/login">
            <label>Password<input name="password" type="password" autocomplete="current-password" required /></label>
            <button type="submit">Log in</button>
          </form>
        "#))
    }
}

#[post("/admin/posts")]
async fn create_post(form: web::Form<NewPostForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if !is_admin(&session) {
        return redirect("/admin/login");
    }
    let title = form.title.trim();
    let body = form.body.trim();
    if title.is_empty() || body.is_empty() {
        return HttpResponse::BadRequest().body("Title and post text are required.");
    }
    let mut post = Post { slug: String::new(), title: title.to_owned(), date: form.date.clone(), body: body.to_owned() };
    let mut posts = match state.posts.lock() {
        Ok(posts) => posts,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let mut updated_posts = posts.clone();
    post.slug = unique_slug(&post.date, &post.title, &updated_posts);
    updated_posts.push(post.clone());
    updated_posts.sort_by(|a, b| b.date.cmp(&a.date));
    if let Err(error) = save_post(&state.posts_dir, &post) {
        return HttpResponse::InternalServerError().body(format!("Could not save post: {error}"));
    }
    *posts = updated_posts;
    redirect("/admin")
}

#[post("/admin/logout")]
async fn logout(session: Session) -> impl Responder {
    session.purge();
    redirect("/admin/login")
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    // Load local development secrets while allowing real environment variables to override them.
    let _ = dotenvy::dotenv();
    let admin_password = env::var("BLOG_ADMIN_PASSWORD")
        .map_err(|_| io::Error::other("Set BLOG_ADMIN_PASSWORD before starting the server."))?;
    if admin_password.is_empty() {
        return Err(io::Error::other("BLOG_ADMIN_PASSWORD cannot be empty."));
    }
    let session_key = env::var("BLOG_SESSION_KEY")
        .map_err(|_| io::Error::other("Set BLOG_SESSION_KEY to a random 64-byte value before starting the server."))?;
    if session_key.as_bytes().len() < 64 {
        return Err(io::Error::other("BLOG_SESSION_KEY must be at least 64 bytes."));
    }

    let posts_dir = PathBuf::from("data");
    fs::create_dir_all(&posts_dir)?;
    let legacy_json = posts_dir.join("blog_posts.json");
    let posts = load_posts(&posts_dir, &legacy_json)?;
    let state = web::Data::new(AppState { posts: Mutex::new(posts), posts_dir, admin_password });
    let key = Key::from(session_key.as_bytes());

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_owned())
        .parse::<u16>()
        .map_err(|error| io::Error::other(format!("PORT must be a valid port number: {error}")))?;
    println!("Resume site running on 0.0.0.0:{port}");
    println!("Blog admin at /admin");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(SessionMiddleware::builder(CookieSessionStore::default(), key.clone())
                .cookie_secure(!cfg!(debug_assertions))
                .build())
            .service(home)
            .service(projects)
            .service(theme_script)
            .service(health)
            .service(fhir_image)
            .service(hl7_image)
            .service(interface_image)
            .service(blog)
            .service(blog_post)
            .service(admin)
            .service(login_page)
            .service(login)
            .service(create_post)
            .service(logout)
    })
        .bind(("0.0.0.0", port))?
        .run()
        .await
}

fn is_admin(session: &Session) -> bool {
    session.get::<bool>("is_admin").ok().flatten().unwrap_or(false)
}

fn redirect(path: &str) -> HttpResponse {
    HttpResponse::SeeOther().insert_header((header::LOCATION, path)).finish()
}

fn read_posts(state: &web::Data<AppState>) -> Vec<Post> {
    state.posts.lock().map(|posts| posts.clone()).unwrap_or_default()
}

fn read_posts_from_file() -> Vec<Post> {
    let directory = PathBuf::from("data");
    load_posts(&directory, &directory.join("blog_posts.json")).unwrap_or_default()
}

fn load_posts(directory: &Path, legacy_json: &Path) -> io::Result<Vec<Post>> {
    let mut posts = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") { continue; }
        if let Ok(contents) = fs::read_to_string(path) {
            if let Some(post) = parse_markdown_post(&contents) { posts.push(post); }
        }
    }
    // Migrate the existing JSON archive once, retaining it as a backup.
    if posts.is_empty() {
        let legacy_posts = fs::read_to_string(legacy_json).ok()
            .and_then(|contents| serde_json::from_str::<Vec<Post>>(&contents).ok())
            .unwrap_or_default();
        posts = legacy_posts;
    }
    posts.sort_by(|a, b| b.date.cmp(&a.date));
    let mut used: Vec<String> = Vec::new();
    for index in 0..posts.len() {
        let post = &posts[index];
        if post.slug.trim().is_empty() || used.contains(&post.slug) {
            let base = format!("{}-{}", slugify(&post.date), slugify(&post.title));
            let base = base.trim_matches('-').to_owned();
            let base = if base.is_empty() { "post".to_owned() } else { base };
            let mut candidate = base.clone(); let mut suffix = 2;
            while used.contains(&candidate) { candidate = format!("{base}-{suffix}"); suffix += 1; }
            posts[index].slug = candidate;
        }
        used.push(posts[index].slug.clone());
    }
    let has_markdown_posts = fs::read_dir(directory)?.filter_map(Result::ok)
        .any(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"));
    if !has_markdown_posts {
        for post in &posts { save_post(directory, post)?; }
    }
    Ok(posts)
}

fn save_post(directory: &Path, post: &Post) -> io::Result<()> {
    let title = serde_json::to_string(&post.title).map_err(io::Error::other)?;
    let date = serde_json::to_string(&post.date).map_err(io::Error::other)?;
    let slug = serde_json::to_string(&post.slug).map_err(io::Error::other)?;
    let contents = format!("---\ntitle: {title}\ndate: {date}\nslug: {slug}\n---\n\n{}\n", post.body);
    fs::write(directory.join(format!("{}.md", post.slug)), contents)
}

fn parse_markdown_post(contents: &str) -> Option<Post> {
    let rest = contents.strip_prefix("---\n")?;
    let (front_matter, body) = rest.split_once("\n---\n")?;
    let mut title = None; let mut date = None; let mut slug = None;
    for line in front_matter.lines() {
        let (key, value) = line.split_once(':')?;
        let value = value.trim();
        let parsed = serde_json::from_str::<String>(value).unwrap_or_else(|_| value.trim_matches('"').to_owned());
        match key.trim() { "title" => title = Some(parsed), "date" => date = Some(parsed), "slug" => slug = Some(parsed), _ => {} }
    }
    Some(Post { title: title?, date: date?, slug: slug.unwrap_or_default(), body: body.strip_prefix('\n').unwrap_or(body).trim_end().to_owned() })
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() { format!("{preview}…") } else { preview }
}

fn escape_html(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&#39;")
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() { slug.push(ch); dash = false; }
        else if !dash && !slug.is_empty() { slug.push('-'); dash = true; }
    }
    slug.trim_matches('-').to_owned()
}

fn unique_slug(date: &str, title: &str, posts: &[Post]) -> String {
    let base = format!("{}-{}", slugify(date), slugify(title)).trim_matches('-').to_owned();
    let base = if base.is_empty() { "post".to_owned() } else { base };
    let mut candidate = base.clone();
    let mut index = 2;
    while posts.iter().any(|post| post.slug == candidate) {
        candidate = format!("{base}-{index}");
        index += 1;
    }
    candidate
}

fn render_post_card(post: &Post) -> String {
    let title = escape_html(&post.title);
    let date = escape_html(&post.date);
    let summary = escape_html(&preview(&post.body, 180));
    format!("<article class=\"post\" data-search=\"{}\"><time>{date}</time><h2><a href=\"/blog/{}\">{title}</a></h2><p>{summary}</p><a class=\"read-more\" href=\"/blog/{}\">Read post →</a></article>", escape_html(&format!("{} {} {}", post.title, post.date, post.body).to_lowercase()), escape_html(&post.slug), escape_html(&post.slug))
}

fn render_markdown(source: &str) -> String {
    let mut html = String::new();
    let mut paragraph = Vec::new();
    let mut in_list = false;
    let mut in_code = false;
    let flush_paragraph = |html: &mut String, lines: &mut Vec<String>| {
        if !lines.is_empty() { html.push_str(&format!("<p>{}</p>", render_inline(&lines.join(" ")))); lines.clear(); }
    };
    for line in source.lines() {
        if line.trim_start().starts_with("```") {
            flush_paragraph(&mut html, &mut paragraph);
            if in_list { html.push_str("</ul>"); in_list = false; }
            if in_code { html.push_str("</code></pre>"); } else { html.push_str("<pre><code>"); }
            in_code = !in_code;
            continue;
        }
        if in_code { html.push_str(&escape_html(line)); html.push('\n'); continue; }
        let trimmed = line.trim();
        if trimmed.is_empty() { flush_paragraph(&mut html, &mut paragraph); if in_list { html.push_str("</ul>"); in_list = false; } continue; }
        let heading_level = trimmed.chars().take_while(|c| *c == '#').count();
        if heading_level > 0 && heading_level <= 6 && trimmed[heading_level..].starts_with(' ') {
            flush_paragraph(&mut html, &mut paragraph); if in_list { html.push_str("</ul>"); in_list = false; }
            html.push_str(&format!("<h{heading_level}>{}</h{heading_level}>", render_inline(trimmed[heading_level..].trim())));
        } else if let Some(quote) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut html, &mut paragraph); if in_list { html.push_str("</ul>"); in_list = false; }
            html.push_str(&format!("<blockquote><p>{}</p></blockquote>", render_inline(quote)));
        } else if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            flush_paragraph(&mut html, &mut paragraph); if !in_list { html.push_str("<ul>"); in_list = true; }
            html.push_str(&format!("<li>{}</li>", render_inline(item)));
        } else { if in_list { html.push_str("</ul>"); in_list = false; } paragraph.push(trimmed.to_owned()); }
    }
    flush_paragraph(&mut html, &mut paragraph);
    if in_list { html.push_str("</ul>"); }
    if in_code { html.push_str("</code></pre>"); }
    html
}

fn render_inline(source: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&['*','*']) {
            if let Some(end) = (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j..].starts_with(&['*','*'])) {
                out.push_str("<strong>"); out.push_str(&escape_html(&chars[i+2..end].iter().collect::<String>())); out.push_str("</strong>"); i = end + 2; continue;
            }
        }
        let ch = chars[i];
        if ch == '`' { if let Some(end) = chars[i+1..].iter().position(|c| *c == '`').map(|p| i+1+p) { out.push_str("<code>"); out.push_str(&escape_html(&chars[i+1..end].iter().collect::<String>())); out.push_str("</code>"); i=end+1; continue; } }
        if ch == '*' || ch == '_' { if let Some(end) = chars[i+1..].iter().position(|c| *c == ch).map(|p| i+1+p) { out.push_str("<em>"); out.push_str(&escape_html(&chars[i+1..end].iter().collect::<String>())); out.push_str("</em>"); i=end+1; continue; } }
        if ch == '[' { if let Some(close) = chars[i+1..].iter().position(|c| *c == ']').map(|p| i+1+p) { if close+1 < chars.len() && chars[close+1] == '(' { if let Some(end) = chars[close+2..].iter().position(|c| *c == ')').map(|p| close+2+p) { let label = escape_html(&chars[i+1..close].iter().collect::<String>()); let url = chars[close+2..end].iter().collect::<String>(); if url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:") { out.push_str(&format!("<a href=\"{}\" rel=\"noopener noreferrer\">{label}</a>", escape_html(&url))); i=end+1; continue; } } } } }
        out.push_str(&escape_html(&ch.to_string())); i += 1;
    }
    out
}

fn admin_page(content: &str) -> String {
    format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><script>try{{const t=localStorage.getItem('theme');if(t)document.documentElement.dataset.theme=t}}catch{{}}</script><title>Blog Admin — Benjamin Larkey</title><style>
      :root{{--ink:#202c2a;--muted:#687773;--accent:#0066cc;--red:#d93645;--paper:#fafbf9;--line:#e2e8e4;--surface:#fff;color-scheme:light}}:root[data-theme="dark"]{{--ink:#ecf3fb;--muted:#a5b2c0;--accent:#59a6ff;--red:#ff6675;--paper:#101923;--line:#2a3948;--surface:#172431;color-scheme:dark}}*{{box-sizing:border-box}}body{{margin:0;background:var(--paper);color:var(--ink);font:16px/1.6 system-ui,sans-serif}}a{{color:inherit}}.page{{width:min(100% - 40px,680px);margin:0 auto}}header{{display:flex;align-items:center;justify-content:space-between;padding:26px 0;border-bottom:1px solid var(--line)}}header a{{font-weight:700;text-decoration:none}}main{{padding:60px 0 80px}}.eyebrow{{color:var(--accent);font-size:.78rem;font-weight:700;letter-spacing:.08em;text-transform:uppercase}}h1{{margin:10px 0 24px;font-size:clamp(2.4rem,7vw,3.7rem);line-height:1.05;letter-spacing:-.06em}}h1::after{{display:block;width:42px;height:3px;margin-top:16px;background:linear-gradient(90deg,var(--accent) 0 72%,var(--red) 72%);content:""}}h2{{margin-top:38px;font-size:1.15rem}}.muted{{color:var(--muted)}}.error{{color:var(--red)}}form{{display:grid;gap:16px}}label{{display:grid;gap:6px;font-size:.9rem;font-weight:600}}input,textarea{{width:100%;padding:11px 12px;border:1px solid var(--line);border-radius:5px;background:var(--surface);color:var(--ink);font:inherit}}textarea{{resize:vertical}}button{{justify-self:start;padding:10px 16px;border:0;border-radius:4px;background:var(--accent);color:white;font:inherit;font-weight:650;cursor:pointer}}button:hover{{filter:brightness(.9)}}.theme-toggle{{padding:7px 10px;background:transparent;border:1px solid var(--line);border-radius:5px;color:var(--ink);font-size:.84rem;font-weight:550}}.theme-toggle:hover{{background:var(--surface)}}.post-list{{padding-left:20px;color:var(--muted)}}.post-list li{{padding:7px 0}}.post-list span{{margin-left:12px;font-size:.85rem}}.logout{{margin-top:24px}}button.quiet{{padding:0;background:transparent;color:var(--muted);font-weight:500}}footer{{padding:22px 0 35px;border-top:1px solid var(--line);color:var(--muted);font-size:.85rem}}
    </style></head><body><div class="page"><header><a href="/">Benjamin Larkey</a><button class="theme-toggle" type="button" aria-label="Switch color theme">☾ Dark mode</button></header><main>{content}</main><footer><a href="/blog">← View blog</a></footer></div><script src="/theme.js" defer></script></body></html>"#)
}
