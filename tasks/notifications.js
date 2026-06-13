export async function sendNotification(site, previousStatus, nextStatus) {
  const provider = site.notification_provider;
  const webhookUrl = site.webhook_url;
  if (!provider || !webhookUrl) {
    return;
  }

  const emoji = nextStatus === "up" ? ":white_check_mark:" : ":x:";
  const text = `${emoji} *${site.name}* is *${nextStatus.toUpperCase()}* (${site.url})`;

  let body;
  let headers = { "Content-Type": "application/json" };

  if (provider === "slack") {
    body = JSON.stringify({ text });
  } else if (provider === "discord") {
    body = JSON.stringify({ content: text.replace(/\*/g, "**") });
  } else {
    return;
  }

  const response = await fetch(webhookUrl, {
    method: "POST",
    headers,
    body,
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(`Webhook failed (${response.status}): ${detail.slice(0, 200)}`);
  }
}
