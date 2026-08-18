COPY (SELECT DISTINCT logradouro FROM read_parquet('https://github.com/ipeaGIT/padronizacao_cnefe/releases/download/v0.4.1/municipio_logradouro_numero_localidade.parquet') ORDER BY 1) 
TO 'dados/snapshot_test/logradouro_referencia.csv' (HEADER false, DELIMITER '', QUOTE '', ESCAPE '', FORMAT CSV);

COPY (SELECT DISTINCT numero FROM read_parquet('https://github.com/ipeaGIT/padronizacao_cnefe/releases/download/v0.4.1/municipio_logradouro_numero_localidade.parquet') ORDER BY 1) 
TO 'dados/snapshot_test/numero_referencia.csv' (HEADER false, DELIMITER '', QUOTE '', ESCAPE '', FORMAT CSV);

COPY (SELECT DISTINCT localidade FROM read_parquet('https://github.com/ipeaGIT/padronizacao_cnefe/releases/download/v0.4.1/municipio_logradouro_numero_localidade.parquet') ORDER BY 1) 
TO 'dados/snapshot_test/localidade_referencia.csv' (HEADER false, DELIMITER '', QUOTE '', ESCAPE '', FORMAT CSV);

COPY (SELECT DISTINCT municipio FROM read_parquet('https://github.com/ipeaGIT/padronizacao_cnefe/releases/download/v0.4.1/municipio_logradouro_numero_localidade.parquet') ORDER BY 1) 
TO 'dados/snapshot_test/municipio_referencia.csv' (HEADER false, DELIMITER '', QUOTE '', ESCAPE '', FORMAT CSV);
