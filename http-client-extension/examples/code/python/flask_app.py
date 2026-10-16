# Flask.
# Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
from flask import Flask, request
from flask_login import login_required

app = Flask(__name__)


@app.route("/articles")
def list_articles():
    page = request.args.get("page")
    return []


@app.route("/articles/<int:article_id>", methods=["PUT"])
def update_article(article_id):
    title = request.json["title"]
    body = request.json.get("body")
    return {}


@app.route("/articles", methods=["POST"])
@login_required
def create_article():
    tenant = request.headers.get("X-Tenant")
    return {}, 201
