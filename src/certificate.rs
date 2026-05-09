use image::ImageReader;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Cursor, io::Write as _, path::Path as StdPath};

#[derive(Serialize)]
pub(crate) struct Certificate {
    pub(crate) cert_id: String,
    pub(crate) employee_name: String,
    pub(crate) issued_at_utc: String,
    pub(crate) score_percent: f32,
    pub(crate) score: usize,
    pub(crate) total: usize,
    pub(crate) digest: String,
    pub(crate) verification_code: String,
}

pub(crate) trait CertificateOperations {
    fn build_certificate(
        &self,
        cert_id: &str,
        employee_name: &str,
        score: usize,
        total: usize,
    ) -> Certificate;

    async fn write_certificate_files(
        &self,
        cert_dir: &StdPath,
        cert: &Certificate,
    ) -> std::io::Result<()>;

    fn verification_code(
        &self,
        cert_id: &str,
        employee_name: &str,
        score: usize,
        total: usize,
    ) -> String;
}

pub(crate) struct CertificateService;

impl CertificateOperations for CertificateService {
    fn build_certificate(
        &self,
        cert_id: &str,
        employee_name: &str,
        score: usize,
        total: usize,
    ) -> Certificate {
        let digest = format!(
            "{:x}",
            Sha256::digest(format!("{cert_id}:{employee_name}:{score}:{total}"))
        );
        Certificate {
            cert_id: cert_id.to_owned(),
            employee_name: employee_name.to_owned(),
            issued_at_utc: chrono::Utc::now().to_rfc3339(),
            score_percent: (score as f32 / total as f32) * 100.0,
            score,
            total,
            digest,
            verification_code: self.verification_code(cert_id, employee_name, score, total),
        }
    }

    async fn write_certificate_files(
        &self,
        cert_dir: &StdPath,
        cert: &Certificate,
    ) -> std::io::Result<()> {
        let badge_path = StdPath::new("resources").join("badge.png");
        let badge_bytes = tokio::fs::read(&badge_path).await?;
        let cert_json = serde_json::to_string_pretty(cert)
            .map_err(|err| std::io::Error::other(format!("serialization error: {err}")))?;
        let json_path = cert_dir.join(format!("certificate-{}.json", cert.cert_id));
        tokio::fs::write(json_path, cert_json).await?;
        let pdf_path = cert_dir.join(format!("certificate-{}.pdf", cert.cert_id));
        tokio::fs::write(pdf_path, build_certificate_pdf(cert, &badge_bytes)?).await
    }

    fn verification_code(
        &self,
        cert_id: &str,
        employee_name: &str,
        score: usize,
        total: usize,
    ) -> String {
        let digest = Sha256::digest(format!("verify:{cert_id}:{employee_name}:{score}:{total}"));
        let hex = format!("{:x}", digest);
        hex[..12].to_uppercase()
    }
}

fn escape_pdf_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[allow(
    clippy::too_many_lines,
    reason = "The handcrafted PDF template is intentionally kept in one place to preserve certificate layout."
)]
fn build_certificate_pdf(cert: &Certificate, badge_png: &[u8]) -> std::io::Result<Vec<u8>> {
    let content = format!(
        "q
1 1 1 rg
0 0 792 612 re
f
Q
\
q
1 1 1 rg
0 0 792 612 re
f
Q
\
q
1 1 1 rg
0 0 792 612 re
f
Q
\
q
1 1 1 rg
28 28 736 556 re
f
Q
\
q
0.73 0.82 0.96 rg
40 40 712 532 re
S
Q
\
q
0.65 0.76 0.95 rg
52 52 688 508 re
S
Q
\
q
0.84 0.91 1 rg
56 56 680 500 re
f
Q
\
q
0.75 0.86 0.99 rg
70 70 652 472 re
f
Q
\
q
0.82 0.90 0.99 rg
80 500 632 3 re
f
80 118 632 3 re
f
Q
\
q
0.79 0.87 0.98 rg
120 450 560 2 re
f
120 170 560 2 re
f
Q
\
q
0.20 0.35 0.66 RG
6 w
30 30 732 552 re
S
Q
\
q
0.27 0.46 0.80 RG
2 w
48 48 696 516 re
S
Q
\
q
0.76 0.65 0.37 RG
6 w
30 30 732 552 re
S
Q
\
q
0.86 0.77 0.50 RG
2 w
48 48 696 516 re
S
Q
\
q
170 0 0 170 545 85 cm
/Im1 Do
Q
\
BT
/F1 38 Tf
80 500 Td
(Completion Certificate) Tj
\
0 -44 Td
/F1 16 Tf
(Awarded for successful completion of Annual Software Development Training) Tj
\
0 -68 Td
/F1 20 Tf
(Presented to) Tj
\
0 -42 Td
/F1 28 Tf
({}) Tj
\
0 -54 Td
/F1 18 Tf
(Completed At \\(UTC\\): {}) Tj
\
0 -30 Td
(Certificate ID: {}) Tj
\
0 -30 Td
(Score: {}/{} \\({:.1}%\\)) Tj
\
0 -30 Td
(Verification Code: {}) Tj
\
0 -52 Td
/F1 13 Tf
(Use certificate ID and verification code to confirm completion.) Tj
ET
",
        escape_pdf_text(&cert.employee_name),
        escape_pdf_text(&cert.issued_at_utc),
        escape_pdf_text(&cert.cert_id),
        cert.score,
        cert.total,
        cert.score_percent,
        escape_pdf_text(&cert.verification_code),
    );

    let (badge_width, badge_height, badge_stream, badge_alpha_stream) =
        encode_badge_streams(badge_png)?;
    let mut pdf = Vec::new();
    pdf.extend_from_slice(
        b"%PDF-1.4
",
    );
    let mut offsets = vec![0_usize];

    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 792 612] /Resources << /Font << /F1 4 0 R >> /XObject << /Im1 6 0 R >> >> /Contents 5 0 R >>
endobj
");
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"4 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
endobj
",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        format!(
            "5 0 obj
<< /Length {} >>
stream
{}endstream
endobj
",
            content.len(),
            content
        )
        .as_bytes(),
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        format!(
            "6 0 obj
<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask 7 0 R /Filter /FlateDecode /Length {} >>
stream
",
            badge_width,
            badge_height,
            badge_stream.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&badge_stream);
    pdf.extend_from_slice(
        b"
endstream
endobj
",
    );

    offsets.push(pdf.len());
    pdf.extend_from_slice(
        format!(
            "7 0 obj
<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>
stream
",
            badge_width,
            badge_height,
            badge_alpha_stream.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&badge_alpha_stream);
    pdf.extend_from_slice(
        b"
endstream
endobj
",
    );

    let xref_start = pdf.len();
    pdf.extend_from_slice(
        b"xref
0 8
0000000000 65535 f 
",
    );
    for off in offsets.iter().skip(1) {
        pdf.extend_from_slice(
            format!(
                "{:010} 00000 n 
",
                off
            )
            .as_bytes(),
        );
    }
    pdf.extend_from_slice(
        b"trailer
<< /Size 8 /Root 1 0 R >>
",
    );
    pdf.extend_from_slice(
        format!(
            "startxref
{}
%%EOF
",
            xref_start
        )
        .as_bytes(),
    );
    Ok(pdf)
}

fn encode_badge_streams(png_bytes: &[u8]) -> std::io::Result<(u32, u32, Vec<u8>, Vec<u8>)> {
    let image = ImageReader::new(Cursor::new(png_bytes))
        .with_guessed_format()
        .map_err(|err| std::io::Error::other(format!("badge format error: {err}")))?
        .decode()
        .map_err(|err| std::io::Error::other(format!("badge decode error: {err}")))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let (rgb, alpha): (Vec<u8>, Vec<u8>) =
        image
            .into_raw()
            .chunks_exact(4)
            .fold((Vec::new(), Vec::new()), |mut channels, pixel| {
                channels.0.extend_from_slice(&pixel[..3]);
                channels.1.push(pixel[3]);
                channels
            });
    let mut rgb_encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    rgb_encoder
        .write_all(&rgb)
        .map_err(|err| std::io::Error::other(format!("badge stream write error: {err}")))?;
    let compressed_rgb = rgb_encoder
        .finish()
        .map_err(|err| std::io::Error::other(format!("badge compression error: {err}")))?;
    let mut alpha_encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    alpha_encoder
        .write_all(&alpha)
        .map_err(|err| std::io::Error::other(format!("badge alpha stream write error: {err}")))?;
    let compressed_alpha = alpha_encoder
        .finish()
        .map_err(|err| std::io::Error::other(format!("badge alpha compression error: {err}")))?;
    Ok((width, height, compressed_rgb, compressed_alpha))
}
