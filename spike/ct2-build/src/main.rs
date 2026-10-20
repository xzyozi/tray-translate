//! Issue #5: ct2rs(CTranslate2)と SentencePiece がビルド・リンクできるかの確認用。
//! 翻訳は行わない。ビルドが通り、実行ファイルが起動することだけを確認する。

use ct2rs::Config;

fn main() {
    // ct2rs の型を実際に参照し、CTranslate2 のリンクまで通ることを確認する
    let _config = Config::default();

    println!("ct2rs build check: ok");
    println!("backend ruy={}", cfg!(feature = "ruy"));
    println!("backend dnnl={}", cfg!(feature = "dnnl"));
    println!("backend mkl={}", cfg!(feature = "mkl"));
    println!("backend openmp={}", cfg!(feature = "openmp"));
}
