import https from "node:https";
import selfsigned from "selfsigned";

const port = Number(process.env.PORT || 8443);
const attrs = [{ name: "commonName", value: "mock-https" }];
const pems = selfsigned.generate(attrs, { days: 365, keySize: 2048 });

const server = https.createServer(
  { key: pems.private, cert: pems.cert },
  (req, res) => {
    const match = req.url?.match(/^\/status\/(\d{3})$/);
    const status = match ? Number(match[1]) : 404;
    res.writeHead(status, { "Content-Type": "text/plain" });
    res.end(`mock status ${status}\n`);
  },
);

server.listen(port, "0.0.0.0", () => {
  console.log(`mock-https listening on https://0.0.0.0:${port}`);
});
