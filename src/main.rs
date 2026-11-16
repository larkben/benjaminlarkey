use std::{env, fs, io, path::PathBuf, sync::Mutex};

use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    App, HttpResponse, HttpServer, Responder, cookie::Key, get, http::header, post, web,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Post {
    title: String,
    date: String,
    body: String,
}

struct AppState {
    posts: Mutex<Vec<Post>>,
    posts_path: PathBuf,
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
            format!("<article class=\"feed-post\"><a href=\"/blog\"><span class=\"terminal-date\">[{date}]</span><strong>{title}</strong><span class=\"terminal-preview\">{preview}</span></a></article>")
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
        posts.iter().map(render_post).collect::<String>()
    };
    let html = include_str!("blog.html").replace("{{POSTS}}", &rendered_posts);
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
        <label>Post<textarea name="body" rows="9" required></textarea></label>
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
    let post = Post { title: title.to_owned(), date: form.date.clone(), body: body.to_owned() };
    let mut posts = match state.posts.lock() {
        Ok(posts) => posts,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let mut updated_posts = posts.clone();
    updated_posts.push(post);
    updated_posts.sort_by(|a, b| b.date.cmp(&a.date));
    if let Err(error) = save_posts(&state.posts_path, &updated_posts) {
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

    let posts_path = PathBuf::from("data/blog_posts.json");
    if let Some(parent) = posts_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !posts_path.exists() {
        fs::write(&posts_path, "[]")?;
    }
    let posts = load_posts(&posts_path);
    let state = web::Data::new(AppState { posts: Mutex::new(posts), posts_path, admin_password });
    let key = Key::from(session_key.as_bytes());

    println!("Resume site running at http://127.0.0.1:8080");
    println!("Blog admin at http://127.0.0.1:8080/admin");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(SessionMiddleware::builder(CookieSessionStore::default(), key.clone())
                .cookie_secure(!cfg!(debug_assertions))
                .build())
            .service(home)
            .service(projects)
            .service(theme_script)
            .service(fhir_image)
            .service(hl7_image)
            .service(interface_image)
            .service(blog)
            .service(admin)
            .service(login_page)
            .service(login)
            .service(create_post)
            .service(logout)
    })
        .bind(("127.0.0.1", 8080))?
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
    let path = PathBuf::from("data/blog_posts.json");
    load_posts(&path)
}

fn load_posts(path: &PathBuf) -> Vec<Post> {
    fs::read_to_string(path).ok()
        .and_then(|contents| serde_json::from_str::<Vec<Post>>(&contents).ok())
        .map(|mut posts| { posts.sort_by(|a, b| b.date.cmp(&a.date)); posts })
        .unwrap_or_default()
}

fn save_posts(path: &PathBuf, posts: &[Post]) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(posts).map_err(io::Error::other)?;
    fs::write(path, json)
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

fn render_post(post: &Post) -> String {
    format!("<article class=\"post\"><time>{}</time><h2>{}</h2><p>{}</p></article>",
        escape_html(&post.date), escape_html(&post.title), escape_html(&post.body))
}

fn admin_page(content: &str) -> String {
    format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><script>try{{const t=localStorage.getItem('theme');if(t)document.documentElement.dataset.theme=t}}catch{{}}</script><title>Blog Admin — Benjamin Larkey</title><style>
      :root{{--ink:#202c2a;--muted:#687773;--accent:#0066cc;--red:#d93645;--paper:#fafbf9;--line:#e2e8e4;--surface:#fff;color-scheme:light}}:root[data-theme="dark"]{{--ink:#ecf3fb;--muted:#a5b2c0;--accent:#59a6ff;--red:#ff6675;--paper:#101923;--line:#2a3948;--surface:#172431;color-scheme:dark}}*{{box-sizing:border-box}}body{{margin:0;background:var(--paper);color:var(--ink);font:16px/1.6 system-ui,sans-serif}}a{{color:inherit}}.page{{width:min(100% - 40px,680px);margin:0 auto}}header{{display:flex;align-items:center;justify-content:space-between;padding:26px 0;border-bottom:1px solid var(--line)}}header a{{font-weight:700;text-decoration:none}}main{{padding:60px 0 80px}}.eyebrow{{color:var(--accent);font-size:.78rem;font-weight:700;letter-spacing:.08em;text-transform:uppercase}}h1{{margin:10px 0 24px;font-size:clamp(2.4rem,7vw,3.7rem);line-height:1.05;letter-spacing:-.06em}}h1::after{{display:block;width:42px;height:3px;margin-top:16px;background:linear-gradient(90deg,var(--accent) 0 72%,var(--red) 72%);content:""}}h2{{margin-top:38px;font-size:1.15rem}}.muted{{color:var(--muted)}}.error{{color:var(--red)}}form{{display:grid;gap:16px}}label{{display:grid;gap:6px;font-size:.9rem;font-weight:600}}input,textarea{{width:100%;padding:11px 12px;border:1px solid var(--line);border-radius:5px;background:var(--surface);color:var(--ink);font:inherit}}textarea{{resize:vertical}}button{{justify-self:start;padding:10px 16px;border:0;border-radius:4px;background:var(--accent);color:white;font:inherit;font-weight:650;cursor:pointer}}button:hover{{filter:brightness(.9)}}.theme-toggle{{padding:7px 10px;background:transparent;border:1px solid var(--line);border-radius:5px;color:var(--ink);font-size:.84rem;font-weight:550}}.theme-toggle:hover{{background:var(--surface)}}.post-list{{padding-left:20px;color:var(--muted)}}.post-list li{{padding:7px 0}}.post-list span{{margin-left:12px;font-size:.85rem}}.logout{{margin-top:24px}}button.quiet{{padding:0;background:transparent;color:var(--muted);font-weight:500}}footer{{padding:22px 0 35px;border-top:1px solid var(--line);color:var(--muted);font-size:.85rem}}
    </style></head><body><div class="page"><header><a href="/">Benjamin Larkey</a><button class="theme-toggle" type="button" aria-label="Switch color theme">☾ Dark mode</button></header><main>{content}</main><footer><a href="/blog">← View blog</a></footer></div><script src="/theme.js" defer></script></body></html>"#)
}
