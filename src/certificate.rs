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

pub(crate) fn build_certificate(
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
        verification_code: verification_code(cert_id, employee_name, score, total),
    }
}

pub(crate) async fn write_certificate_files(
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

pub(crate) fn verification_code(
    cert_id: &str,
    employee_name: &str,
    score: usize,
    total: usize,
) -> String {
    let digest = Sha256::digest(format!("verify:{cert_id}:{employee_name}:{score}:{total}"));
    let hex = format!("{:x}", digest);
    hex[..12].to_uppercase()
}

fn escape_pdf_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn build_certificate_pdf(cert: &Certificate, badge_png: &[u8]) -> std::io::Result<Vec<u8>> {
    let content = format!(
        "BT /F1 12 Tf 80 500 Td ({}) Tj ET",
        escape_pdf_text(&cert.employee_name)
    );
    let (badge_width, badge_height, badge_stream, badge_alpha_stream) =
        encode_badge_streams(badge_png)?;
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = vec![0_usize];
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 792 612] /Resources << /Font << /F1 4 0 R >> /XObject << /Im1 6 0 R >> >> /Contents 5 0 R >>\nendobj\n");
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        format!(
            "5 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
            content.len(),
            content
        )
        .as_bytes(),
    );
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("6 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask 7 0 R /Filter /FlateDecode /Length {} >>\nstream\n", badge_width, badge_height, badge_stream.len()).as_bytes());
    pdf.extend_from_slice(&badge_stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("7 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n", badge_width, badge_height, badge_alpha_stream.len()).as_bytes());
    pdf.extend_from_slice(&badge_alpha_stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let xref_start = pdf.len();
    pdf.extend_from_slice(b"xref\n0 8\n0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    pdf.extend_from_slice(b"trailer\n<< /Size 8 /Root 1 0 R >>\n");
    pdf.extend_from_slice(format!("startxref\n{}\n%%EOF\n", xref_start).as_bytes());
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
