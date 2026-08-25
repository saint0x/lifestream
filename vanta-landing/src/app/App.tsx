import { ArrowRight, ChevronDown, Play, Radio } from "lucide-react";
import { SignupForm } from "@/components/SignupForm";
import { featuredTiles, pages, routeToAudience } from "@/content/pages";
import "@/app/App.css";

export function App() {
  const audience = routeToAudience(window.location.pathname);
  const page = pages[audience];
  const tiles = page.tiles ?? featuredTiles;

  document.title = audience === "home" ? "Vanta | HBO plus Twitch" : `${page.formTitle} | Vanta`;

  return (
    <main className="vl-page">
      <header className="vl-nav">
        <a className="vl-mark" href="/">
          <span />
          Vanta
        </a>
      </header>

      <section className="vl-hero">
        <div className="vl-hero__media" aria-hidden="true">
          <img src={page.image} alt="" />
          <div className="vl-player">
            <div className="vl-player__bar">
              <span />
              <span />
              <span />
            </div>
            <div className="vl-player__center">
              <Play size={30} fill="currentColor" />
            </div>
            <div className="vl-player__meta">
              <span>Streaming now</span>
              <strong>{page.strips[0]}</strong>
            </div>
          </div>
        </div>

        <div className="vl-hero__copy">
          <span className="vl-label">{page.eyebrow}</span>
          <h1>{page.title}</h1>
          <p>{page.subtitle}</p>
          {audience === "home" ? (
            <div className="vl-actions">
              <a className="vl-button vl-button--primary" href="#signup">
                <ArrowRight size={16} />
                {page.primaryCta}
              </a>
              {page.secondaryCta ? (
                <a className="vl-button" href="/creators">
                  <Radio size={16} />
                  {page.secondaryCta}
                </a>
              ) : null}
            </div>
          ) : null}
        </div>

        <a className="vl-cue" href="#proof" aria-label="Skip to proof">
          <ChevronDown size={18} />
        </a>
      </section>

      {page.metrics.length > 0 ? (
        <section className="vl-metrics" aria-label="Vanta metrics">
          {page.metrics.map((metric) => (
            <div key={metric.label} className="vl-metric">
              <span>{metric.label}</span>
              <strong>{metric.value}</strong>
              <p>{metric.detail}</p>
            </div>
          ))}
        </section>
      ) : null}

      <section id="proof" className="vl-proof">
        {page.proof.map((item) => {
          const Icon = item.icon;
          return (
            <article key={item.title}>
              <Icon size={19} />
              <h3>{item.title}</h3>
              <p>{item.body}</p>
            </article>
          );
        })}
      </section>

      {page.steps?.length ? (
        <section className="vl-steps" aria-label="How Vanta works">
          <div className="vl-steps__inner">
            <div className="vl-steps__head">
              <span className="vl-label">How it works</span>
              <h2>{page.stepsTitle}</h2>
              {page.stepsIntro ? <p>{page.stepsIntro}</p> : null}
            </div>
            <ol className="vl-steps__list">
              {page.steps.map((step) => (
                <li key={step.label}>
                  <span>{step.label}</span>
                  <div>
                    <h3>{step.title}</h3>
                    <p>{step.body}</p>
                  </div>
                </li>
              ))}
            </ol>
          </div>
        </section>
      ) : null}

      {tiles.length > 0 ? (
        <section className="vl-gallery" aria-label="Vanta programming and proof">
          {tiles.map((tile) => {
            const Icon = tile.icon;
            return (
              <article key={`${tile.label}-${tile.title}`}>
                <img src={tile.image} alt="" />
                <div>
                  <span><Icon size={14} /> {tile.label}</span>
                  <h3>{tile.title}</h3>
                </div>
              </article>
            );
          })}
        </section>
      ) : null}

      <section className="vl-faq">
        <div>
          <span className="vl-label">FAQ</span>
          <h2>The questions worth answering before you join.</h2>
        </div>
        <div className="vl-faq__list">
          {page.faq.map((item) => (
            <details key={item.question} open={item === page.faq[0]}>
              <summary>{item.question}</summary>
              <p>{item.answer}</p>
            </details>
          ))}
        </div>
      </section>

      <section id="signup" className="vl-signup">
        <SignupForm page={page} />
      </section>
    </main>
  );
}
