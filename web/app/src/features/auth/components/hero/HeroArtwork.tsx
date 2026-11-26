import React, { type PropsWithChildren } from 'react';

export function HeroTagline({
  title = '1flowbase',
  slogan
}: {
  title?: string;
  slogan: string;
}) {
  return (
    <div
      className="hero-text-block"
      style={{
        zIndex: 1,
        textAlign: 'center',
        padding: `0 clamp(12px, 6vw, 80px)`,
        width: '100%',
        marginTop: '-15vh',
        pointerEvents: 'none'
      }}
    >
      <div className="hero-tagline" aria-label="brand name and slogan">
        <div className="hero-text-line hero-title-line">{title}</div>
        <div className="hero-text-line hero-slogan-line">{slogan}</div>
      </div>
    </div>
  );
}

export function HeroArtwork({ children }: PropsWithChildren) {
  return (
    <>
      <canvas
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          width: '100vw',
          height: '100vh',
          pointerEvents: 'none',
          zIndex: 0
        }}
      />

      <div
        className="auth-sign-in-hero"
        style={{
          flex: 1,
          position: 'relative',
          overflow: 'hidden',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center'
        }}
      >
        <div
          className="hero-shape shape-1"
          style={{
            position: 'absolute',
            width: '600px',
            height: '600px',
            borderRadius: '50%',
            background:
              'radial-gradient(circle at center, rgba(0, 208, 132, 0.14) 0%, transparent 70%)',
            top: '-10%',
            left: '-10%',
            filter: 'blur(60px)',
            pointerEvents: 'none'
          }}
        />
        <div
          className="hero-shape shape-2"
          style={{
            position: 'absolute',
            width: '800px',
            height: '800px',
            borderRadius: '50%',
            background:
              'radial-gradient(circle at center, rgba(0, 162, 255, 0.12) 0%, transparent 70%)',
            bottom: '-20%',
            right: '-10%',
            filter: 'blur(80px)',
            pointerEvents: 'none'
          }}
        />
        <div
          className="hero-shape shape-3"
          style={{
            position: 'absolute',
            width: '400px',
            height: '400px',
            borderRadius: '50%',
            background:
              'radial-gradient(circle at center, rgba(52, 211, 153, 0.07) 0%, transparent 70%)',
            top: '40%',
            left: '60%',
            filter: 'blur(50px)',
            pointerEvents: 'none'
          }}
        />

        {children}
        <style>
          {`
          .hero-text-block {
            animation: fadeIn 1.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
          }

          .hero-tagline {
            --hero-slogan-size: clamp(1.2rem, 2.2vw, 2.5rem);
            --hero-title-size: calc(var(--hero-slogan-size) * 2.2);
            display: inline-flex;
            flex-direction: column;
            align-items: center;
            gap: clamp(0.5rem, 1vw, 1.5rem);
            line-height: 1.2;
            font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
          }

          .hero-title-line {
            font-size: var(--hero-title-size);
            font-weight: 800;
            letter-spacing: -0.02em;
            background: linear-gradient(135deg, #a3f5d5 0%, #008f5f 55%, #16211d 95%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
            margin: 0;
            line-height: 1.15;
            filter: drop-shadow(0 0 6px rgba(163, 245, 213, 0.5)) drop-shadow(0 2px 3px rgba(0, 143, 95, 0.3));
          }

          .hero-slogan-line {
            font-size: var(--hero-slogan-size);
            font-weight: 400;
            letter-spacing: 0.06em;
            color: #55645d;
            margin: 0;
            line-height: 1.15;
          }

          @keyframes fadeIn {
            0% { opacity: 0; transform: translateY(15px); }
            100% { opacity: 1; transform: translateY(0); }
          }

          .hero-shape {
            animation: float 25s ease-in-out infinite alternate;
          }
          .shape-1 {
            animation-delay: 0s;
            animation-duration: 25s;
          }
          .shape-2 {
            animation-delay: -5s;
            animation-duration: 35s;
            animation-direction: alternate-reverse;
          }
          .shape-3 {
            animation-delay: -10s;
            animation-duration: 20s;
          }

          @keyframes float {
            0% {
              transform: translate(0, 0) scale(1);
            }
            33% {
              transform: translate(25px, -40px) scale(1.05);
            }
            66% {
              transform: translate(-15px, 15px) scale(0.95);
            }
            100% {
              transform: translate(0, 0) scale(1);
            }
          }
        `}
        </style>
      </div>
    </>
  );
}
